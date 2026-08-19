//! The gallery — bezel's documentation, and its dev surface: a rail of every
//! component on the left, the selected one in the pane.
//!
//! [`TABS`] is the catalog: a top nav for the *kind* of thing, a rail for the
//! items in it. A component lands here the day it lands in `crates/ui`,
//! composed exactly once, so the browser is never out of date with the library
//! it documents.

use gpui::{
    AnyElement, App, Axis, Context, DragMoveEvent, Empty, Entity, KeyBinding, SharedString, Window,
    actions, div, prelude::*, px, relative,
};
use markdown::editor;
use std::{cell::Cell, collections::HashSet, rc::Rc};
use theme::{
    Theme,
    appearance::{self, AppearanceMode},
};
use ui::{
    combobox::{self, Combobox},
    control_bar::Shape as ControlBarShape,
    date::{self, Calendar, Date},
    focus,
    hover_card::HoverCard,
    icons,
    input::{self, Shape, TextField},
    list, loaders,
    menubar::{self, Item, Menu, Menubar, MenubarEvent},
    pagination,
    palette::{self, CommandPalette, PaletteEvent},
    popover,
    scroll::{self, ScrollbarState},
    table::{self, Column, Sort, Width},
    tooltip::Tooltip,
    tree::{self, Direction, Move},
    widgets,
    widgets::{
        ButtonStyle, Buttons, Content, Controls, Layout, Scaffolding, SliderDrag, SplitDrag, Status,
    },
};

actions!(gallery, [OpenPalette, ToggleInspector, ToggleFullScreen]);

/// Every keymap this view needs, in one call.
///
/// Two entry points open it — a native window and a browser tab — and a list
/// each of them keeps by hand is a list they drift out of: the editor's
/// bindings were installed natively and missing on the web, so typing worked in
/// the browser and Backspace did not.
pub fn init(cx: &mut App) {
    markdown::set_highlighter(cx, highlight::spans);
    input::init(cx);
    editor::init(cx);
    palette::init(cx);
    combobox::init(cx);
    date::init(cx);
    focus::init(cx);
    menubar::init(cx);
    tree::init(cx);
    // A pattern is an app: the composer page binds its own keys.
    patterns::agent::init(cx);
    cx.bind_keys([KeyBinding::new("cmd-k", OpenPalette, None)]);
}

pub mod highlight;
/// gpui builds its element inspector into every debug build; release builds
/// have no such window method, so the whole surface is debug-only.
#[cfg(debug_assertions)]
pub mod inspector;

pub mod patterns;

pub const LANGUAGES: [&str; 8] = [
    "Rust",
    "TypeScript",
    "Swift",
    "Zig",
    "Go",
    "Python",
    "Haskell",
    "OCaml",
];

pub const COMMANDS: [&str; 8] = [
    "Open File…",
    "Open Recent",
    "Save All",
    "Toggle Sidebar",
    "Toggle Theme",
    "Reload Window",
    "Copy Path",
    "Quit",
];

const SELECT_CHOICES: [&str; 3] = ["Comfortable", "Compact", "Dense"];

/// What one press of ← or → moves the slider. The step is never the library's:
/// bezel dispatches [`focus::Decrement`]/[`focus::Increment`] and the page that
/// owns the value decides what they are worth.
const SLIDER_STEP: f32 = 0.05;

/// Pages in the pagination page's imaginary result set — the shape of thing
/// that arrives one page at a time and cannot be held whole.
const RESULT_PAGES: usize = 87;

/// How many rows the virtualized-list page claims to hold. Large enough that
/// building them all would be obvious.
const VIRTUAL_ROWS: usize = 10_000;

/// The tree page's data. A nested structure the *app* owns — bezel never sees
/// one, which is why [`Gallery::tree_rows`] below exists.
struct Node {
    name: &'static str,
    children: &'static [Node],
}

const FILE_TREE: &[Node] = &[
    Node {
        name: "crates",
        children: &[
            Node {
                name: "ui",
                children: &[
                    Node {
                        name: "table.rs",
                        children: &[],
                    },
                    Node {
                        name: "tree.rs",
                        children: &[],
                    },
                ],
            },
            Node {
                name: "theme",
                children: &[Node {
                    name: "lib.rs",
                    children: &[],
                }],
            },
        ],
    },
    Node {
        name: "apps",
        children: &[Node {
            name: "gallery",
            children: &[Node {
                name: "main.rs",
                children: &[],
            }],
        }],
    },
    Node {
        name: "README.md",
        children: &[],
    },
];

/// One flattened row: what bezel needs to paint and navigate it, plus what this
/// app needs to identify it again.
struct TreeRow {
    row: tree::Row,
    label: &'static str,
    path: String,
}

/// Flatten the open parts of [`FILE_TREE`] into visible rows.
///
/// This is the function every consumer of `tree` writes, and the reason the
/// module asks for a flat list: bezel cannot walk a tree it knows nothing
/// about, and the app has to produce these rows to render them anyway.
fn flatten_tree(
    nodes: &'static [Node],
    depth: usize,
    prefix: &str,
    expanded: &HashSet<String>,
    out: &mut Vec<TreeRow>,
) {
    for node in nodes {
        let path = if prefix.is_empty() {
            node.name.to_string()
        } else {
            format!("{prefix}/{}", node.name)
        };
        let open = expanded.contains(&path);
        out.push(TreeRow {
            row: if node.children.is_empty() {
                tree::Row::leaf(depth)
            } else {
                tree::Row::branch(depth, open)
            },
            label: node.name,
            path: path.clone(),
        });
        if open {
            flatten_tree(node.children, depth + 1, &path, expanded, out);
        }
    }
}

/// The table page's columns, declared once — the header and every row are laid
/// out from this exact slice, which is what keeps them lined up.
fn table_columns() -> Vec<Column> {
    vec![
        Column::new("Name", Width::Flex(2.0)),
        Column::new("Kind", Width::Flex(1.0)),
        // Right-aligned so the digits line up by place value.
        Column::new("Size", Width::Fixed(px(90.0))).align_end(),
    ]
}

/// Rows for the table page. Made-up file listing rather than anything measured
/// from this repo: a demo that quoted real numbers would be wrong by the next
/// commit and nothing would notice.
const TABLE_ROWS: [(&str, &str, u32); 7] = [
    ("bezel.toml", "Config", 812),
    ("palette.rs", "Source", 24_930),
    ("README.md", "Document", 4_216),
    ("Geist.ttf", "Font", 1_284_400),
    ("icons/", "Folder", 58),
    ("theme.json", "Config", 9_004),
    ("notes.md", "Document", 1_130),
];

/// One row on the Step row page. A build rather than an agent turn, on purpose:
/// the component is named for the shape, and the shape is "an operation with an
/// outcome" wherever it turns up.
struct Step {
    icon: &'static str,
    title: &'static str,
    detail: &'static str,
    meta: &'static str,
    failed: bool,
    /// `None` is a step that printed nothing, which is what suppresses the
    /// chevron.
    output: Option<&'static str>,
}

const STEPS: [Step; 3] = [
    Step {
        icon: icons::TERMINAL,
        title: "cargo test",
        detail: "-p ui",
        meta: "1.4s",
        failed: false,
        output: Some(
            "running 84 tests\n\
             test widgets::the_first_press_flips_what_was_on_screen ... ok\n\
             test scroll::following_means_within_slack_of_the_end ... ok\n\
             \n\
             test result: ok. 84 passed; 0 failed",
        ),
    },
    Step {
        icon: icons::MAGNIFER,
        title: "Search",
        detail: "fn at_bottom",
        meta: "12ms",
        failed: false,
        output: None,
    },
    Step {
        icon: icons::DOCUMENT,
        title: "Read",
        detail: "crates/ui/src/missing.rs",
        meta: "3ms",
        failed: true,
        output: Some("error: no such file or directory (os error 2)"),
    },
];

/// The menubar page's menus. Ordinary app chrome, with the two rows worth
/// showing: a separator, and a disabled item the keyboard steps straight over.
///
/// The accelerators are printed, not bound — `menubar` never dispatches, so
/// these name shortcuts this app would wire itself.
fn demo_menus() -> Vec<Menu> {
    vec![
        Menu::new(
            "File",
            vec![
                Item::action("New Window").with_keystroke("⌘N"),
                Item::action("Open…").with_keystroke("⌘O"),
                Item::Separator,
                Item::action("Save").with_keystroke("⌘S"),
                Item::action("Save As…").with_keystroke("⇧⌘S").disabled(),
            ],
        ),
        Menu::new(
            "Edit",
            vec![
                Item::action("Undo").with_keystroke("⌘Z"),
                Item::action("Redo").with_keystroke("⇧⌘Z").disabled(),
                Item::Separator,
                Item::action("Cut").with_keystroke("⌘X"),
                Item::action("Copy").with_keystroke("⌘C"),
                Item::action("Paste").with_keystroke("⌘V"),
            ],
        ),
        Menu::new(
            "View",
            vec![
                Item::action("Toggle Sidebar").with_keystroke("⌘B"),
                Item::action("Full Screen").with_keystroke("⌃⌘F"),
            ],
        ),
    ]
}

/// One page of the browser.
pub struct Section {
    /// Rail key, and what [`Gallery::section_body`] matches on.
    pub key: &'static str,
    pub title: &'static str,
    /// Where the component is written. Customisation here is editing the
    /// source, so the path is the most useful line of documentation there is.
    /// `None` for a component the rail lists but the library has not built.
    pub source: Option<&'static str>,
}

/// A rail group.
pub struct Group {
    pub title: &'static str,
    pub sections: &'static [Section],
}

const fn section(key: &'static str, title: &'static str, source: &'static str) -> Section {
    Section {
        key,
        title,
        source: Some(source),
    }
}

/// A component the rail lists but the library does not have yet. Its page says
/// what the remaining work is, which is what turns the rail into a measure of
/// what is left rather than a list of what exists.
///
/// Unused as of the pagination commit — every row in the catalog now has a
/// source, which is a milestone rather than a reason to delete the mechanism.
/// The convention (a `planned()` row plus an arm in [`PLANNED_BODIES`], and a
/// test that fails until both are dropped together) is what the next unbuilt
/// component will be declared with.
#[allow(dead_code)]
const fn planned(key: &'static str, title: &'static str) -> Section {
    Section {
        key,
        title,
        source: None,
    }
}

/// A top-nav tab, holding its own rail.
pub struct Tab {
    pub title: &'static str,
    pub groups: &'static [Group],
    /// The page the tab opens on.
    pub home: &'static str,
    /// Whether its pages get the whole pane instead of the fixed column every
    /// component demo is designed for. A pattern is a screen: it fills the
    /// pane, scrolls its own parts, and floats its own chrome over them.
    pub full_bleed: bool,
}

/// The top nav. The axis is the *kind* of thing you are looking at: a token, a
/// component, or a screen built out of both.
pub const TABS: &[Tab] = &[
    Tab {
        title: "Foundations",
        groups: FOUNDATIONS,
        home: "color",
        full_bleed: false,
    },
    Tab {
        title: "Components",
        groups: COMPONENTS,
        home: "buttons",
        full_bleed: false,
    },
    Tab {
        title: "Patterns",
        groups: PATTERNS,
        home: "agent-activity",
        full_bleed: true,
    },
];

/// Composed screens. The source path points at the gallery rather than into
/// `crates/`, and that is the point — a pattern is not a component you call, it
/// is a file you copy.
pub const PATTERNS: &[Group] = &[
    // A group per kind of app, and this one is the driver: bezel is a UI
    // library for agent apps. A page appears here when the parts under it are
    // real — nothing is a row until it can be pressed.
    Group {
        title: "Agent",
        sections: &[
            section(
                "agent-activity",
                "Activity",
                "apps/gallery/src/patterns/agent.rs",
            ),
            section(
                "agent-tools",
                "Tool calls",
                "apps/gallery/src/patterns/agent.rs",
            ),
            section(
                "agent-composer",
                "Composer",
                "apps/gallery/src/patterns/agent.rs",
            ),
            section(
                "agent-transcript",
                "Transcript",
                "apps/gallery/src/patterns/transcript.rs",
            ),
            section("agent-diff", "Diff", "apps/gallery/src/patterns/diff.rs"),
            // Native-only: the terminal crate sits off the wasm build
            // (alacritty_terminal pulls `home`, which does not compile for
            // wasm32) — see the gallery manifest.
            #[cfg(not(target_family = "wasm"))]
            section(
                "agent-terminal",
                "Terminal",
                "apps/gallery/src/patterns/terminal.rs",
            ),
            section(
                "agent-orbs",
                "Thinking orbs",
                "apps/gallery/src/patterns/orbs.rs",
            ),
            section(
                "agent-avatar",
                "Blob avatars",
                "apps/gallery/src/patterns/avatar.rs",
            ),
        ],
    },
    Group {
        title: "Media",
        sections: &[
            section(
                "document",
                "Document",
                "apps/gallery/src/patterns/document.rs",
            ),
            section("syntax", "Syntax", "apps/gallery/src/patterns/syntax.rs"),
        ],
    },
];

/// The layers under the components — what a token *is*, before anything paints
/// with it. The source paths say which crate each belongs to, which is the
/// honest version of navigating by crate.
pub const FOUNDATIONS: &[Group] = &[
    Group {
        title: "Style",
        sections: &[
            section("color", "Color", "crates/theme/src/lib.rs"),
            section("typography", "Typography", "crates/theme/src/lib.rs"),
            section("layout", "Layout", "crates/theme/src/lib.rs"),
            section("material", "Materials", "crates/ui/src/material.rs"),
        ],
    },
    Group {
        title: "Motion",
        sections: &[
            section("motion-curves", "Curves", "crates/motion/src/lib.rs"),
            section("motion-catalog", "Catalog", "crates/motion/src/lib.rs"),
        ],
    },
    Group {
        title: "Assets",
        sections: &[section("icons", "Icons", "crates/ui/src/icons.rs")],
    },
];

/// The rail, grouped by what a component is *for* — Apple's HIG split rather
/// than shadcn's flat alphabetical list, because Law 2 already says this
/// library speaks SwiftUI.
///
/// This is the list: adding a component means one row here and one arm in
/// [`Gallery::section_body`].
pub const COMPONENTS: &[Group] = &[
    Group {
        title: "Selection & input",
        sections: &[
            section("buttons", "Buttons", "crates/ui/src/widgets/buttons.rs"),
            section("text-field", "Text field", "crates/ui/src/input.rs"),
            section("textarea", "Textarea", "crates/ui/src/input.rs"),
            section("select", "Select", "crates/ui/src/widgets/controls.rs"),
            section("combobox", "Combobox", "crates/ui/src/combobox.rs"),
            section(
                "checkbox-radio",
                "Checkbox & radio",
                "crates/ui/src/widgets/controls.rs",
            ),
            section("toggle", "Toggle", "crates/ui/src/widgets/controls.rs"),
            section(
                "toggle-group",
                "Toggle group",
                "crates/ui/src/widgets/controls.rs",
            ),
            section("slider", "Slider", "crates/ui/src/widgets/controls.rs"),
            section("date-picker", "Date picker", "crates/ui/src/date.rs"),
        ],
    },
    Group {
        title: "Menus & actions",
        sections: &[
            section("menu", "Menu", "crates/ui/src/popover.rs"),
            section("context-menu", "Context menu", "crates/ui/src/popover.rs"),
            section("palette", "Command palette", "crates/ui/src/palette.rs"),
            section("menubar", "Menubar", "crates/ui/src/menubar.rs"),
        ],
    },
    Group {
        title: "Presentation",
        sections: &[
            section("dialog", "Dialog", "crates/ui/src/popover.rs"),
            section("sheet", "Sheet", "crates/ui/src/popover.rs"),
            section("tooltip", "Tooltip", "crates/ui/src/tooltip.rs"),
            section("hover-card", "Hover card", "crates/ui/src/hover_card.rs"),
        ],
    },
    Group {
        title: "Layout & organisation",
        sections: &[
            section(
                "group-box",
                "Group box",
                "crates/ui/src/widgets/scaffolding.rs",
            ),
            section("tabs", "Tabs", "crates/ui/src/widgets/layout.rs"),
            section(
                "collapsible",
                "Collapsible",
                "crates/ui/src/widgets/layout.rs",
            ),
            section(
                "split",
                "Resizable split",
                "crates/ui/src/widgets/layout.rs",
            ),
            section("control-bar", "Control bar", "crates/ui/src/control_bar.rs"),
        ],
    },
    // Nothing here is built. It is one whole group on purpose: the data
    // surfaces were deferred together, and they are the next round together.
    Group {
        title: "Data",
        sections: &[
            section("scroll-area", "Scroll area", "crates/ui/src/scroll.rs"),
            section("follow", "Follow scroll", "crates/ui/src/scroll.rs"),
            section("table", "Table", "crates/ui/src/table.rs"),
            section("tree", "Tree view", "crates/ui/src/tree.rs"),
            section("virtual-list", "Virtualized list", "crates/ui/src/list.rs"),
        ],
    },
    Group {
        title: "Content",
        sections: &[
            section("badge", "Badge", "crates/ui/src/widgets/content.rs"),
            section("tag", "Tag", "crates/ui/src/widgets/content.rs"),
            section("avatar", "Avatar", "crates/ui/src/widgets/content.rs"),
            section(
                "breadcrumb",
                "Breadcrumb",
                "crates/ui/src/widgets/content.rs",
            ),
            section("pagination", "Pagination", "crates/ui/src/pagination.rs"),
            section(
                "empty-state",
                "Empty state",
                "crates/ui/src/widgets/content.rs",
            ),
            section("skeleton", "Skeleton", "crates/ui/src/popover.rs"),
        ],
    },
    Group {
        title: "Status",
        sections: &[
            section("progress", "Progress", "crates/ui/src/widgets/controls.rs"),
            section("status-dot", "Status dot", "crates/ui/src/widgets/mod.rs"),
            section("alerts", "Alert strips", "crates/ui/src/widgets/status.rs"),
            section("step-row", "Step row", "crates/ui/src/widgets/status.rs"),
            section("loaders", "Loaders", "crates/ui/src/loaders.rs"),
        ],
    },
];

/// The keys [`Gallery::section_body`] answers with a TODO page. Listed rather
/// than derived because the arms cannot be enumerated at runtime; a test keeps
/// this in step with the [`planned`] rows.
pub const PLANNED_BODIES: &[&str] = &[];

fn section_at(key: &str) -> Option<&'static Section> {
    TABS.iter()
        .flat_map(|tab| tab.groups)
        .flat_map(|group| group.sections)
        .find(|section| section.key == key)
}

/// Which tab holds a key, so an embed can select a page without knowing the
/// shape of the catalog above it.
fn tab_of(key: &str) -> Option<usize> {
    TABS.iter().position(|tab| {
        tab.groups
            .iter()
            .any(|group| group.sections.iter().any(|section| section.key == key))
    })
}

pub struct Gallery {
    search: Entity<TextField>,
    filled: Entity<TextField>,
    /// The two multi-line shapes. Their row counts are this page's example, not
    /// a default the library holds — `Shape` takes them from the caller.
    notes: Entity<TextField>,
    composer: Entity<TextField>,
    /// Mounted only while open — a palette that lingers keeps a stale query.
    palette: Option<Entity<CommandPalette>>,
    last_command: Option<SharedString>,
    segment: usize,
    expanded: bool,
    /// Which step rows are showing their output.
    step_open: [bool; 3],
    /// The second collapsible: a section that follows a run until you take it
    /// over. `running` is what a streaming flag would be in a real app.
    running: bool,
    details: widgets::Takeover,
    /// Right-click menu, anchored at the click position.
    context_menu: popover::Popup<gpui::Point<gpui::Pixels>>,
    /// Select state lives here, not in a component: the menu is mounted by
    /// this view, so this view owns whether it is open and what is chosen.
    theme_menu: popover::Popup<()>,
    theme_choice: usize,
    /// The combobox, by contrast, owns its own menu — it has a query field to
    /// hold, so it is an entity.
    language: Entity<Combobox>,
    /// So does the date picker, which holds a month and a cursor.
    date: Entity<Calendar>,
    /// And the menubar, which holds which menu is down.
    menubar: Entity<Menubar>,
    /// What it last reported. The bar keeps no selection — a menu item is an
    /// action, not a value — so the host is where the answer lands.
    last_menu_item: Option<SharedString>,
    sheet: popover::Popup<()>,
    /// Where the split's divider sits, as a fraction of the container.
    split: f32,
    split_dragging: bool,
    /// The window's resting focus. Without it the key context has no node in
    /// the focus path, and `cmd-k` reaches nothing.
    focus_handle: gpui::FocusHandle,
    /// Focus for the wired controls. A stateless `fn(&Theme, ..) -> Div` has
    /// nowhere to keep a handle, so the view that composes it holds them —
    /// the same place it already holds what each one is set to.
    buttons: [gpui::FocusHandle; 3],
    checkboxes: [gpui::FocusHandle; 2],
    radios: [gpui::FocusHandle; 2],
    switches: [gpui::FocusHandle; 2],
    segments: [gpui::FocusHandle; 3],
    slider: gpui::FocusHandle,
    tab_strip: [gpui::FocusHandle; 3],
    /// Which button was last pressed, and by what — the only way to see that a
    /// keyboard press and a click reach the same place.
    last_pressed: Option<SharedString>,
    /// What the wired controls are set to. Every one of them paints from the
    /// caller's state and reports nothing back, so this is where the answer is.
    checked: [bool; 2],
    radio: usize,
    switched: [bool; 2],
    level: f32,
    tab_choice: usize,
    /// Scroll position and thumb-grab for every scrolling surface here. gpui
    /// owns the offset; the second half of each pair is only where in the thumb
    /// a drag took hold.
    rail_scroll: gpui::ScrollHandle,
    rail_bar: ScrollbarState,
    pane_scroll: gpui::ScrollHandle,
    pane_bar: ScrollbarState,
    demo_scroll: gpui::ScrollHandle,
    demo_bar: ScrollbarState,
    /// The follow-scroll demo: a log that grows under a view pinned to its end.
    log_scroll: gpui::ScrollHandle,
    log_bar: ScrollbarState,
    log_follow: scroll::FollowState,
    log_lines: usize,
    table_scroll: gpui::ScrollHandle,
    table_bar: ScrollbarState,
    tree_scroll: gpui::ScrollHandle,
    tree_bar: ScrollbarState,
    rows_scroll: gpui::UniformListScrollHandle,
    rows_bar: ScrollbarState,
    /// How many of [`VIRTUAL_ROWS`] rows the list actually built last frame.
    /// A `Cell` because the count is written from inside the render closure,
    /// which the list owns and calls with no view in scope — and it is the only
    /// honest way to *show* that virtualization is happening.
    rows_built: Rc<Cell<usize>>,
    /// Which folders are open, by path. App data, and the reason `tree` reports
    /// an intent rather than expanding anything itself.
    tree_expanded: HashSet<String>,
    tree_selected: Option<String>,
    tree_cursor: usize,
    tree_focus: gpui::FocusHandle,
    /// Which page of the imaginary result set is showing. 1-based, like the
    /// component: it is a label, not an index.
    page: usize,
    /// Which column the table page is sorted by. The app's, because the app is
    /// what has to sort the rows — the table only says what a click meant.
    table_sort: Option<Sort>,
    /// One field per pattern, because a pattern is a screen and owns a screen's
    /// worth of state. A component demo can keep its value or two up here
    /// beside the rest; thirteen of them cannot.
    activity: Entity<patterns::agent::Activity>,
    tool_calls: Entity<patterns::agent::ToolCalls>,
    agent_composer: Entity<patterns::agent::Composer>,
    transcript: Entity<patterns::transcript::Transcript>,
    diff: Entity<patterns::diff::Diff>,
    document: Entity<patterns::document::Document>,
    #[cfg(not(target_family = "wasm"))]
    terminal: Entity<patterns::terminal::Terminal>,
    orbs: Entity<patterns::orbs::Orbs>,
    syntax: Entity<patterns::syntax::Syntax>,
    avatar: Entity<patterns::avatar::Avatars>,
    /// Which top-nav tab is open.
    tab: usize,
    /// Where you were in each tab — switching away and back should land you
    /// where you left, not at the top.
    selected: Vec<&'static str>,
    dialog: popover::Popup<()>,
    /// Renders one section alone, without the nav, rail or header around it.
    /// The website embeds a page per component this way, so a doc page shows
    /// the component it documents rather than the whole browser.
    embedded: bool,
}

impl Gallery {
    pub fn new(cx: &mut Context<Self>) -> Self {
        // The bar reports a place in the menus it was given; the host is what
        // turns that back into a name, and what decides it means anything.
        let menubar = cx.new(|cx| Menubar::new(demo_menus(), cx));
        cx.subscribe(&menubar, |view, bar, event, cx| {
            let MenubarEvent::Selected { menu, item } = event;
            if let Some(Item::Action { label, .. }) = bar.read(cx).menus()[*menu].items.get(*item) {
                view.last_menu_item = Some(label.clone());
            }
            cx.notify();
        })
        .detach();

        Self {
            menubar,
            last_menu_item: None,
            search: cx.new(|cx| TextField::new(cx).with_placeholder("Search components…")),
            filled: cx.new(|cx| {
                let mut field = TextField::new(cx);
                field.set_content("Select me with shift-left", cx);
                field
            }),
            notes: cx.new(|cx| {
                let mut field = TextField::new(cx).with_shape(Shape::Rows(4));
                field.set_content(
                    "Wrapping is the point: this line is longer than the box, so it \
                     folds. Press enter for a hard break.",
                    cx,
                );
                field
            }),
            composer: cx.new(|cx| {
                TextField::new(cx)
                    .with_shape(Shape::Grow { min: 2, max: 6 })
                    .with_placeholder("Grows as you type…")
            }),
            theme_menu: popover::Popup::default(),
            theme_choice: 0,
            language: cx.new(|cx| {
                Combobox::new(
                    LANGUAGES.iter().map(|l| SharedString::from(*l)).collect(),
                    "Pick a language",
                    cx,
                )
                .with_selection(0)
            }),
            date: cx.new(|cx| Calendar::new(today(), cx)),
            palette: None,
            last_command: None,
            segment: 0,
            expanded: true,
            step_open: [false; 3],
            // Arrives mid-run, which is the state the auto-follow is for.
            running: true,
            details: widgets::Takeover::default(),
            context_menu: popover::Popup::default(),
            sheet: popover::Popup::default(),
            split: 0.4,
            split_dragging: false,
            focus_handle: cx.focus_handle(),
            buttons: [cx.focus_handle(), cx.focus_handle(), cx.focus_handle()],
            checkboxes: [cx.focus_handle(), cx.focus_handle()],
            radios: [cx.focus_handle(), cx.focus_handle()],
            switches: [cx.focus_handle(), cx.focus_handle()],
            segments: [cx.focus_handle(), cx.focus_handle(), cx.focus_handle()],
            slider: cx.focus_handle(),
            tab_strip: [cx.focus_handle(), cx.focus_handle(), cx.focus_handle()],
            rail_scroll: gpui::ScrollHandle::new(),
            rail_bar: ScrollbarState::new(),
            pane_scroll: gpui::ScrollHandle::new(),
            pane_bar: ScrollbarState::new(),
            demo_scroll: gpui::ScrollHandle::new(),
            demo_bar: ScrollbarState::new(),
            log_scroll: gpui::ScrollHandle::new(),
            log_bar: ScrollbarState::new(),
            log_follow: scroll::FollowState::new(),
            // Enough to overflow the box on arrival, so the pin has something
            // to hold onto before you press anything.
            log_lines: 24,
            table_scroll: gpui::ScrollHandle::new(),
            table_bar: ScrollbarState::new(),
            table_sort: None,
            page: 1,
            tree_scroll: gpui::ScrollHandle::new(),
            tree_bar: ScrollbarState::new(),
            rows_scroll: gpui::UniformListScrollHandle::new(),
            rows_bar: ScrollbarState::new(),
            rows_built: Rc::new(Cell::new(0)),
            // Opened so the page shows nesting on arrival rather than a flat
            // list of two folders.
            tree_expanded: ["crates", "crates/ui"]
                .into_iter()
                .map(String::from)
                .collect(),
            tree_selected: None,
            tree_cursor: 0,
            tree_focus: cx.focus_handle().tab_stop(true),
            last_pressed: None,
            checked: [true, false],
            radio: 0,
            switched: [true, false],
            level: 0.5,
            tab_choice: 0,
            tab: 2,
            selected: TABS
                .iter()
                .enumerate()
                .map(|(i, tab)| if i == 2 { "agent-diff" } else { tab.home })
                .collect(),
            dialog: popover::Popup::default(),
            activity: cx.new(|_| patterns::agent::Activity::default()),
            tool_calls: cx.new(|_| patterns::agent::ToolCalls::default()),
            agent_composer: cx.new(patterns::agent::Composer::new),
            transcript: cx.new(|_| patterns::transcript::Transcript::default()),
            diff: cx.new(|_| patterns::diff::Diff),
            document: cx.new(patterns::document::Document::new),
            #[cfg(not(target_family = "wasm"))]
            terminal: cx.new(patterns::terminal::Terminal::new),
            orbs: cx.new(patterns::orbs::Orbs::new),
            syntax: cx.new(patterns::syntax::Syntax::new),
            avatar: cx.new(patterns::avatar::Avatars::new),
            embedded: false,
        }
    }

    /// One section, alone, for a website page that documents it. Falls back to
    /// the whole browser when the key is not in the catalog, so a stale link
    /// lands somewhere useful instead of on an empty pane.
    pub fn embedded(key: &str, cx: &mut Context<Self>) -> Self {
        let mut gallery = Self::new(cx);
        if let Some(tab) = tab_of(key) {
            gallery.tab = tab;
            gallery.selected[tab] = section_at(key).expect("tab_of matched").key;
            gallery.embedded = true;
        }
        gallery
    }

    /// The gallery's own focus handle — what the window focuses on launch.
    pub fn focus_handle(&self) -> gpui::FocusHandle {
        self.focus_handle.clone()
    }

    fn toggle_theme_menu(&mut self, cx: &mut Context<Self>) {
        // `note_trigger_press` was recorded on mouse-down; if the menu was
        // already open then this click is a dismiss, not a re-open.
        if self.theme_menu.take_press_was_open() {
            if self.theme_menu.begin_close() {
                popover::reap_popup(cx, |view: &mut Self| &mut view.theme_menu);
            }
        } else {
            self.theme_menu.open(());
        }
        cx.notify();
    }

    fn open_palette(&mut self, _: &OpenPalette, window: &mut Window, cx: &mut Context<Self>) {
        let palette = cx.new(|cx| {
            CommandPalette::new(
                COMMANDS.iter().map(|c| SharedString::from(*c)).collect(),
                cx,
            )
        });
        // The host decides what a selection means; the palette only reports.
        cx.subscribe(&palette, |view, _, event, cx| {
            match event {
                PaletteEvent::Selected(index) => {
                    view.last_command = Some(SharedString::from(COMMANDS[*index]));
                }
                PaletteEvent::Dismissed => {}
            }
            view.palette = None;
            cx.notify();
        })
        .detach();
        palette.update(cx, |palette, cx| palette.focus(window, cx));
        self.palette = Some(palette);
        cx.notify();
    }

    /// `cmd-alt-i`. The handler lives on this view rather than on the app so
    /// it has the window in hand — `App::active_window` is a guess, and it is
    /// `None` whenever the app is not frontmost.
    ///
    /// Debug builds only: `Window::toggle_inspector` does not exist in release,
    /// so the whole affordance compiles out.
    fn toggle_inspector(
        &mut self,
        _: &ToggleInspector,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        #[cfg(debug_assertions)]
        _window.toggle_inspector(_cx);
    }

    /// `ctrl-cmd-f`. macOS draws that shortcut on the Window menu of a nib-built
    /// app; a gpui app has no nib, so AppKit supplies nothing and the keystroke
    /// reaches whatever the app binds — which, until this existed, was nothing.
    /// Zed carries the same binding in its own keymap for the same reason.
    fn toggle_full_screen(
        &mut self,
        _: &ToggleFullScreen,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.toggle_fullscreen();
    }

    /// What a button does, reached by click and by `enter`/`space` alike.
    fn press(&mut self, label: &'static str, cx: &mut Context<Self>) {
        self.last_pressed = Some(SharedString::from(label));
        cx.notify();
    }

    /// The visible rows, rebuilt from this view's own tree and its own set of
    /// open folders.
    fn tree_rows(&self) -> Vec<TreeRow> {
        let mut rows = Vec::new();
        flatten_tree(FILE_TREE, 0, "", &self.tree_expanded, &mut rows);
        rows
    }

    /// An arrow key. `tree::step` decides what it meant; applying it is this
    /// view's job, because the set of open folders is this view's.
    fn tree_step(&mut self, direction: Direction, cx: &mut Context<Self>) {
        let rows = self.tree_rows();
        let shape: Vec<tree::Row> = rows.iter().map(|entry| entry.row).collect();
        match tree::step(&shape, self.tree_cursor, direction) {
            Some(Move::To(index)) => self.tree_cursor = index,
            Some(Move::Expand(index)) => {
                self.tree_expanded.insert(rows[index].path.clone());
            }
            Some(Move::Collapse(index)) => {
                self.tree_expanded.remove(&rows[index].path);
            }
            None => {}
        }
        cx.notify();
    }

    /// A click: a folder opens or closes, a file is chosen. Both move the
    /// keyboard cursor, so the two ways of getting around agree on where you
    /// are.
    fn tree_click(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let rows = self.tree_rows();
        let Some(entry) = rows.get(index) else { return };
        self.tree_cursor = index;
        if entry.row.expanded.is_some() {
            if !self.tree_expanded.remove(&entry.path) {
                self.tree_expanded.insert(entry.path.clone());
            }
        } else {
            self.tree_selected = Some(entry.path.clone());
        }
        window.focus(&self.tree_focus, cx);
        cx.notify();
    }

    /// Go to a page, clamped the way the component clamps what it draws — the
    /// prev/next steps hand this `page - 1` and `page + 1` without checking.
    fn go_to_page(&mut self, page: usize, cx: &mut Context<Self>) {
        self.page = page.clamp(1, RESULT_PAGES);
        cx.notify();
    }

    /// A heading was clicked. `next_sort` says what that means; sorting the
    /// rows is this view's job, since they are this view's rows.
    fn sort_table(&mut self, column: usize, cx: &mut Context<Self>) {
        self.table_sort = Some(table::next_sort(self.table_sort, column));
        cx.notify();
    }

    /// The slider by keyboard. Clamped here rather than in the widget: the
    /// paint clamps what it draws, but the value is this view's to keep sane.
    fn nudge(&mut self, delta: f32, cx: &mut Context<Self>) {
        self.level = (self.level + delta).clamp(0.0, 1.0);
        cx.notify();
    }

    fn close_context_menu(&mut self, cx: &mut Context<Self>) {
        if self.context_menu.begin_close() {
            popover::reap_popup(cx, |view: &mut Self| &mut view.context_menu);
        }
        cx.notify();
    }

    fn close_dialog(&mut self, cx: &mut Context<Self>) {
        if self.dialog.begin_close() {
            popover::reap_popup(cx, |view: &mut Self| &mut view.dialog);
        }
        cx.notify();
    }

    fn close_sheet(&mut self, cx: &mut Context<Self>) {
        if self.sheet.begin_close() {
            popover::reap_popup(cx, |view: &mut Self| &mut view.sheet);
        }
        cx.notify();
    }

    fn choose_theme(&mut self, index: usize, cx: &mut Context<Self>) {
        self.theme_choice = index;
        if self.theme_menu.begin_close() {
            popover::reap_popup(cx, |view: &mut Self| &mut view.theme_menu);
        }
        cx.notify();
    }

    /// The navigation rail: every component, one row each, the current one
    /// carrying the same selected wash a menu row does — this is the library
    /// browsing itself.
    fn rail(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let tab = &TABS[self.tab];
        let selected = self.selected[self.tab];
        div()
            .relative()
            .flex_none()
            .w(px(220.0))
            .h_full()
            .border_r_1()
            .border_color(theme.border)
            .child(
                div()
                    .id("gallery-rail")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.rail_scroll)
                    .child(div().flex().flex_col().gap(px(2.0)).p(px(10.0)).children(
                        tab.groups.iter().flat_map(|group| {
                            let heading =
                                popover::menu_heading(theme, group.title).into_any_element();
                            let rows = group.sections.iter().map(|section| {
                                popover::menu_row(
                                    theme,
                                    section.key == selected,
                                    SharedString::from(format!("rail-{}", section.key)),
                                )
                                .id(SharedString::from(format!("rail-item-{}", section.key)))
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    let tab = view.tab;
                                    view.selected[tab] = section.key;
                                    cx.notify();
                                }))
                                // Unbuilt rows stay legible but recede, so the rail
                                // reads as "what exists" and "what is left" at once.
                                .when(section.source.is_none() && section.key != selected, |row| {
                                    row.text_color(theme.text_faint)
                                })
                                .child(SharedString::from(section.title))
                                .into_any_element()
                            });
                            std::iter::once(heading).chain(rows)
                        }),
                    )),
            )
            // After the content: hitboxes and paint are both order-dependent in
            // gpui, so a bar added first would sit under what it reports on.
            .child(scroll::scrollbar(
                "rail-bar",
                &self.rail_scroll,
                &self.rail_bar,
            ))
            .into_any_element()
    }

    /// The top nav: the wordmark, the kind of thing you are browsing, and the
    /// appearance switch. Everything here is global — per-page detail belongs
    /// in [`Self::header`].
    fn nav(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let current = self.tab;
        let dark = matches!(theme.appearance, theme::Appearance::Dark);
        div()
            .flex_none()
            .h(px(Theme::HEADER_HEIGHT))
            .px(px(16.0))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(18.0))
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .px(px(8.0))
                    .text_size(px(14.0))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("bezel"),
            )
            .children(TABS.iter().enumerate().map(|(index, tab)| {
                let selected = index == current;
                let mut item = div()
                    .id(SharedString::from(format!("nav-{}", tab.title)))
                    .px(px(4.0))
                    .py(px(4.0))
                    .text_size(px(13.0))
                    .cursor_pointer()
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.tab = index;
                        cx.notify();
                    }))
                    .child(SharedString::from(tab.title));
                item = if selected {
                    item.font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(theme.text)
                } else {
                    item.text_color(theme.text_muted)
                };
                item.into_any_element()
            }))
            // Pushes the appearance switch to the trailing edge.
            .child(div().flex_1())
            // A switch, not three segments. It reads the *resolved* appearance
            // rather than the mode, so it shows what you are actually looking
            // at while the app is still following the OS — and the first flip
            // is what pins it. Returning to `System` is `set_mode`, which is a
            // settings-level action rather than a nav-level one.
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(icons::icon(icons::SUN).size(px(14.0)).text_color(if dark {
                        theme.text_faint
                    } else {
                        theme.text
                    }))
                    // id + click on the switch itself, not on a wrapper around
                    // it: a `div().id(..)` wrapped around a control takes
                    // clicks over a box narrower than what it paints, which is
                    // the open hit-testing bug in this tree.
                    .child(
                        theme
                            .toggle(dark)
                            .id("appearance")
                            .cursor_pointer()
                            .on_click(cx.listener(move |_, _, _, cx| {
                                appearance::set_mode(
                                    if dark {
                                        AppearanceMode::Light
                                    } else {
                                        AppearanceMode::Dark
                                    },
                                    cx,
                                );
                                cx.notify();
                            })),
                    )
                    .child(icons::icon(icons::MOON).size(px(14.0)).text_color(if dark {
                        theme.text
                    } else {
                        theme.text_faint
                    })),
            )
            .into_any_element()
    }

    /// The bar over the pane: what you are looking at, and where it is written.
    fn header(&self, section: &'static Section, theme: &Theme) -> AnyElement {
        div()
            .flex_none()
            .h(px(Theme::HEADER_HEIGHT))
            .px(px(24.0))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(16.0))
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_baseline()
                    .gap(px(10.0))
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(15.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(SharedString::from(section.title)),
                    )
                    // Customisation is editing the source, so say which file —
                    // unless there is no file, which is worth saying too.
                    .child(
                        match section.source {
                            Some(path) => div()
                                .min_w_0()
                                .truncate()
                                .font_family(theme.font_mono.clone())
                                .child(SharedString::from(path)),
                            None => div().child("not built yet"),
                        }
                        .text_size(px(11.5))
                        .text_color(theme.text_faint),
                    ),
            )
            .into_any_element()
    }

    /// One section by key. Unknown keys render nothing — [`SECTIONS`] is the
    /// list, and anything off it is a typo at the call site.
    fn section_body(&mut self, key: &str, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::of(cx).clone();
        let view = cx.entity_id();
        let section = stack();

        match key {
            // ---- Foundations -------------------------------------------------
            "color" => section
                .child(hint(
                    &theme,
                    "Tokens read at paint time from the theme global — the switch \
                     above swaps every one of them.",
                ))
                .children(color_groups(&theme).into_iter().map(|(title, tokens)| {
                    stack()
                        .child(popover::menu_heading(&theme, title))
                        .child(div().flex().flex_row().flex_wrap().gap(px(10.0)).children(
                            tokens.into_iter().map(|(name, color)| {
                                swatch(&theme, name, color).into_any_element()
                            }),
                        ))
                        .into_any_element()
                }))
                .into_any_element(),

            "typography" => section
                .child(hint(
                    &theme,
                    "Geist and Geist Mono ship with the crate; the sizes below are \
                     the ones the library actually paints.",
                ))
                .child(popover::menu_heading(&theme, "Families"))
                .child(
                    stack()
                        .child(type_row(&theme, theme.font_sans.clone(), "font_sans"))
                        .child(type_row(&theme, theme.font_mono.clone(), "font_mono")),
                )
                .child(popover::menu_heading(&theme, "Weights"))
                .child(
                    stack().children(
                        [
                            (gpui::FontWeight::NORMAL, "NORMAL 400"),
                            (gpui::FontWeight::MEDIUM, "MEDIUM 500"),
                            (gpui::FontWeight::SEMIBOLD, "SEMIBOLD 600"),
                            (gpui::FontWeight::BOLD, "BOLD 700"),
                        ]
                        .into_iter()
                        .map(|(weight, label)| {
                            div()
                                .text_size(px(15.0))
                                .font_weight(weight)
                                .child(SharedString::from(label))
                                .into_any_element()
                        }),
                    ),
                )
                .child(popover::menu_heading(&theme, "Sizes in use"))
                .child(stack().children(TYPE_SCALE.iter().map(|size| {
                    div()
                        .flex()
                        .flex_row()
                        .items_baseline()
                        .gap(px(12.0))
                        .child(
                            div()
                                .w(px(40.0))
                                .flex_none()
                                .text_size(px(11.0))
                                .font_family(theme.font_mono.clone())
                                .text_color(theme.text_faint)
                                .child(SharedString::from(format!("{size}"))),
                        )
                        .child(
                            div()
                                .text_size(px(*size))
                                .child("The quick brown fox jumps"),
                        )
                        .into_any_element()
                })))
                .into_any_element(),

            "layout" => section
                .child(hint(
                    &theme,
                    "Law 4: numbers drive layout, colours are paint. These are the \
                     numbers — plain constants on Theme, no colour involved.",
                ))
                .child(popover::menu_heading(&theme, "Space"))
                .child(
                    stack().children(
                        [
                            ("SPACE_XS", Theme::SPACE_XS),
                            ("SPACE_SM", Theme::SPACE_SM),
                            ("SPACE_MD", Theme::SPACE_MD),
                            ("SPACE_LG", Theme::SPACE_LG),
                        ]
                        .into_iter()
                        .map(|(name, value)| measure(&theme, name, value).into_any_element()),
                    ),
                )
                .child(popover::menu_heading(&theme, "Radius"))
                .child(
                    div().flex().flex_row().gap(px(12.0)).children(
                        [
                            ("CONTROL", Theme::CONTROL_RADIUS),
                            ("PANEL", Theme::PANEL_RADIUS),
                            ("BUBBLE", Theme::BUBBLE_RADIUS),
                        ]
                        .into_iter()
                        .map(|(name, value)| {
                            stack()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .size(px(56.0))
                                        .rounded(px(value))
                                        .border_1()
                                        .border_color(theme.border)
                                        .bg(theme.surface_raised),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .font_family(theme.font_mono.clone())
                                        .text_color(theme.text_faint)
                                        .child(SharedString::from(format!("{name} {value}"))),
                                )
                                .into_any_element()
                        }),
                    ),
                )
                .child(popover::menu_heading(&theme, "Chrome heights"))
                .child(
                    stack().children(
                        [
                            ("HEADER_HEIGHT", Theme::HEADER_HEIGHT),
                            ("TITLEBAR_HEIGHT", Theme::TITLEBAR_HEIGHT),
                            ("STATUS_STRIP_HEIGHT", Theme::STATUS_STRIP_HEIGHT),
                        ]
                        .into_iter()
                        .map(|(name, value)| measure(&theme, name, value).into_any_element()),
                    ),
                )
                .into_any_element(),

            "motion-curves" => section
                .child(hint(
                    &theme,
                    "Plotted from each curve's own `progress()` — the same pure \
                     function the animations run on.",
                ))
                .child(
                    div().flex().flex_row().flex_wrap().gap(px(16.0)).children(
                        [
                            ("EASE", motion::EASE),
                            ("EASE_OUT", motion::EASE_OUT),
                            ("EASE_OUT_EXPO", motion::EASE_OUT_EXPO),
                            ("EASE_IN_OUT", motion::EASE_IN_OUT),
                            ("EASE_RESORT", motion::EASE_RESORT),
                            ("EASE_TAILWIND", motion::EASE_TAILWIND),
                        ]
                        .into_iter()
                        .map(|(name, curve)| {
                            curve_plot(&theme, name, |t| curve.eval(t)).into_any_element()
                        }),
                    ),
                )
                .into_any_element(),

            "motion-catalog" => section
                .child(hint(
                    &theme,
                    "Every named spec. Law 3: no component may inline a duration \
                     or a curve — it names one of these.",
                ))
                .child(stack().children(MOTION_CATALOG.iter().map(|(name, spec)| {
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(12.0))
                        .child(
                            div()
                                .w(px(150.0))
                                .flex_none()
                                .text_size(px(12.5))
                                .font_family(theme.font_mono.clone())
                                .child(SharedString::from(*name)),
                        )
                        .child(
                            div()
                                .w(px(70.0))
                                .flex_none()
                                .text_size(px(11.5))
                                .text_color(theme.text_faint)
                                .child(SharedString::from(if spec.delay_ms > 0 {
                                    format!("{}+{}ms", spec.delay_ms, spec.duration_ms)
                                } else {
                                    format!("{}ms", spec.duration_ms)
                                })),
                        )
                        .child(curve_plot(&theme, "", |t| spec.progress(t)))
                        .into_any_element()
                })))
                .into_any_element(),

            "icons" => section
                .child(hint(
                    &theme,
                    "Solar Icons (Linear) by 480 Design, CC BY 4.0, plus a few drawn \
                     to match. Every one is embedded in the crate.",
                ))
                .child(div().flex().flex_row().flex_wrap().gap(px(8.0)).children(
                    icons::ALL.iter().map(|(name, path)| {
                        div()
                            .w(px(96.0))
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(6.0))
                            .py(px(10.0))
                            .rounded(px(Theme::CONTROL_RADIUS))
                            .border_1()
                            .border_color(theme.border)
                            .child(
                                icons::icon(path)
                                    .size(px(18.0))
                                    .text_color(theme.text_muted),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .truncate()
                                    .text_size(px(9.5))
                                    .text_align(gpui::TextAlign::Center)
                                    .font_family(theme.font_mono.clone())
                                    .text_color(theme.text_faint)
                                    .child(SharedString::from(*name)),
                            )
                            .into_any_element()
                    }),
                ))
                .into_any_element(),

            // ---- Components --------------------------------------------------
            "buttons" => {
                let labels = ["Ghost", "Prominent", "Destructive"];
                let faces = [
                    theme.button(labels[0], ButtonStyle::Ghost, Some("g-ghost".into())),
                    theme.button(labels[1], ButtonStyle::Prominent, None),
                    theme.button(labels[2], ButtonStyle::Destructive, None),
                ];
                section
                    .child(hint(
                        &theme,
                        "tab and shift-tab walk these, and every field and combobox \
                         in the gallery, in the order they are painted. enter or \
                         space presses the focused one.",
                    ))
                    .child(
                        row().children(faces.into_iter().enumerate().map(|(index, face)| {
                            pressable(
                                focus::focusable(&theme, &self.buttons[index], face),
                                SharedString::from(format!("button-{index}")),
                                cx,
                                move |view, cx| view.press(labels[index], cx),
                            )
                            .into_any_element()
                        })),
                    )
                    .when_some(self.last_pressed.clone(), |page, label| {
                        page.child(
                            div()
                                .text_size(px(12.5))
                                .text_color(theme.text_muted)
                                .child(SharedString::from(format!("pressed: {label}"))),
                        )
                    })
                    .into_any_element()
            }

            "text-field" => section
                .child(hint(
                    &theme,
                    "cmd-z undoes a run of typing at a time, not a letter at a time; \
                     moving the caret or switching between typing and deleting ends \
                     the run.",
                ))
                .child(
                    div()
                        .w(px(320.0))
                        .flex()
                        .flex_col()
                        .gap(px(10.0))
                        .child(self.search.clone())
                        .child(self.filled.clone()),
                )
                .into_any_element(),

            "toggle" => section
                .child(hint(&theme, "space or enter flips the focused switch."))
                .child(row().children((0..2).map(|index| {
                    pressable(
                        focus::focusable(
                            &theme,
                            &self.switches[index],
                            theme.toggle(self.switched[index]),
                        ),
                        SharedString::from(format!("toggle-{index}")),
                        cx,
                        move |view, cx| {
                            view.switched[index] = !view.switched[index];
                            cx.notify();
                        },
                    )
                    .into_any_element()
                })))
                .into_any_element(),

            "badge" => section
                .child(
                    row()
                        .child(theme.badge("badge"))
                        .child(theme.badge_active("active")),
                )
                .into_any_element(),

            "select" => {
                let menu_open = self.theme_menu.is_open() || self.theme_menu.is_closing();
                section
                    .child(
                        div().w(px(200.0)).relative().child(
                            div()
                                .id("theme-select")
                                .on_mouse_down(
                                    gpui::MouseButton::Left,
                                    cx.listener(|view, _, _, _| {
                                        view.theme_menu.note_trigger_press()
                                    }),
                                )
                                .on_click(cx.listener(|view, _, _, cx| view.toggle_theme_menu(cx)))
                                .child(
                                    theme.select_trigger(
                                        SELECT_CHOICES[self.theme_choice],
                                        menu_open,
                                    ),
                                )
                                .when(menu_open, |trigger| {
                                    trigger.child(popover::anchored_menu_below(
                                        "theme-select-menu",
                                        popover::popover_card(&theme)
                                            .w(px(200.0))
                                            // Dismissal is the caller's, and the
                                            // caller is this view — without it,
                                            // clicking away leaves it open.
                                            .on_mouse_down_out(cx.listener(|view, _, _, cx| {
                                                if view.theme_menu.begin_close() {
                                                    popover::reap_popup(cx, |view: &mut Self| {
                                                        &mut view.theme_menu
                                                    });
                                                }
                                                cx.notify();
                                            }))
                                            .children(SELECT_CHOICES.iter().enumerate().map(
                                                |(index, label)| {
                                                    popover::menu_row(
                                                        &theme,
                                                        index == self.theme_choice,
                                                        SharedString::from(format!(
                                                            "theme-row-{index}"
                                                        )),
                                                    )
                                                    .id(SharedString::from(format!(
                                                        "theme-{index}"
                                                    )))
                                                    .on_click(cx.listener(move |view, _, _, cx| {
                                                        view.choose_theme(index, cx)
                                                    }))
                                                    .child(*label)
                                                    .into_any_element()
                                                },
                                            ))
                                            .into_any_element(),
                                        self.theme_menu.closing_since(),
                                    ))
                                }),
                        ),
                    )
                    .into_any_element()
            }

            "combobox" => section
                .child(div().w(px(220.0)).child(self.language.clone()))
                .child(
                    div()
                        .text_size(px(12.5))
                        .text_color(theme.text_muted)
                        .child(SharedString::from(
                            match self.language.read(cx).selection() {
                                Some(index) => format!("chosen: {}", LANGUAGES[index]),
                                None => "nothing chosen".to_string(),
                            },
                        )),
                )
                .into_any_element(),

            "checkbox-radio" => section
                .child(hint(
                    &theme,
                    "space or enter flips the focused control. The radios are one \
                     set, so choosing either clears the other; the checkboxes are \
                     two independent answers.",
                ))
                .child(
                    row()
                        .children((0..2).map(|index| {
                            pressable(
                                focus::focusable(
                                    &theme,
                                    &self.checkboxes[index],
                                    theme.checkbox(self.checked[index]),
                                ),
                                SharedString::from(format!("checkbox-{index}")),
                                cx,
                                move |view, cx| {
                                    view.checked[index] = !view.checked[index];
                                    cx.notify();
                                },
                            )
                            .into_any_element()
                        }))
                        .children((0..2).map(|index| {
                            pressable(
                                focus::focusable(
                                    &theme,
                                    &self.radios[index],
                                    theme.radio_button(self.radio == index),
                                ),
                                SharedString::from(format!("radio-{index}")),
                                cx,
                                move |view, cx| {
                                    view.radio = index;
                                    cx.notify();
                                },
                            )
                            .into_any_element()
                        })),
                )
                .into_any_element(),

            "avatar" => section
                .child(row().child(theme.avatar("TC")).child(theme.avatar("K")))
                .into_any_element(),

            "progress" => section
                .child(
                    div()
                        .w(px(280.0))
                        .flex()
                        .flex_col()
                        .gap(px(16.0))
                        .child(theme.progress_bar(0.35))
                        .child(theme.progress_bar(0.8)),
                )
                .into_any_element(),

            "slider" => section
                .child(hint(
                    &theme,
                    "Grab it anywhere and slide, or tab to it and press ← and →.",
                ))
                .child(
                    div().w(px(280.0)).child(
                        focus::focusable(&theme, &self.slider, theme.slider(self.level))
                            .id("slider")
                            // The element is its own drag source, so the gesture
                            // starts wherever the pointer went down on the track.
                            .on_drag(SliderDrag, |_, _, _, cx| cx.new(|_| Empty))
                            .on_drag_move(cx.listener(
                                |view, event: &DragMoveEvent<SliderDrag>, _, cx| {
                                    view.level = widgets::axis_fraction(
                                        event.event.position,
                                        event.bounds,
                                        Axis::Horizontal,
                                        0.0,
                                    );
                                    cx.notify();
                                },
                            ))
                            .on_action(cx.listener(|view, _: &focus::Decrement, _, cx| {
                                view.nudge(-SLIDER_STEP, cx)
                            }))
                            .on_action(cx.listener(|view, _: &focus::Increment, _, cx| {
                                view.nudge(SLIDER_STEP, cx)
                            })),
                    ),
                )
                .child(
                    div()
                        .text_size(px(12.5))
                        .font_family(theme.font_mono.clone())
                        .text_color(theme.text_muted)
                        .child(SharedString::from(format!("{:.0}%", self.level * 100.0))),
                )
                .into_any_element(),

            "toggle-group" => section
                .child(hint(
                    &theme,
                    "One of three: space or enter picks the focused segment.",
                ))
                .child(
                    theme.toggle_group().children(
                        ["Day", "Week", "Month"]
                            .into_iter()
                            .enumerate()
                            .map(|(index, label)| {
                                pressable(
                                    focus::focusable(
                                        &theme,
                                        &self.segments[index],
                                        theme.toggle_group_item(label, self.segment == index),
                                    ),
                                    SharedString::from(format!("segment-{index}")),
                                    cx,
                                    move |view, cx| {
                                        view.segment = index;
                                        cx.notify();
                                    },
                                )
                                .into_any_element()
                            }),
                    ),
                )
                .into_any_element(),

            "collapsible" => {
                let open = self.details.get(self.running);
                section
                    .child(
                        div()
                            .w(px(320.0))
                            .child(
                                div()
                                    .id("collapse")
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.expanded = !view.expanded;
                                        cx.notify();
                                    }))
                                    .child(
                                        theme
                                            .collapsible_header("Advanced", self.expanded)
                                            .hover(widgets::collapsible_header_hover),
                                    ),
                            )
                            .when(self.expanded, |el| {
                                el.child(
                                    div()
                                        .pl(px(24.0))
                                        .pt(px(4.0))
                                        .text_size(px(12.5))
                                        .text_color(theme.text_muted)
                                        .child("Body shown while expanded."),
                                )
                            }),
                    )
                    .child(hint(
                        &theme,
                        "The second one follows the run: it opens itself while \
                         work is streaming in and closes when that stops. Touch \
                         it once and it is yours — start and stop the run after \
                         that and it stays where you put it.",
                    ))
                    .child(
                        row()
                            .child(
                                div()
                                    .id("takeover-run")
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.running = !view.running;
                                        cx.notify();
                                    }))
                                    .child(theme.button(
                                        if self.running {
                                            "Finish the run"
                                        } else {
                                            "Start a run"
                                        },
                                        ButtonStyle::Ghost,
                                        Some("g-takeover-run".into()),
                                    )),
                            )
                            // Which of the two rules is answering, on the page —
                            // the same trick the follow-scroll row uses. A
                            // behaviour you can only infer is one nobody checks.
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_family(theme.font_mono.clone())
                                    .text_color(theme.text_faint)
                                    .child(SharedString::from(format!(
                                        "open: {open} — {}",
                                        if self.details == widgets::Takeover::default() {
                                            "following the run"
                                        } else {
                                            "yours"
                                        }
                                    ))),
                            ),
                    )
                    .child(
                        div()
                            .w(px(320.0))
                            .child(
                                div()
                                    .id("takeover-head")
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        let running = view.running;
                                        view.details.toggle(running);
                                        cx.notify();
                                    }))
                                    .child(
                                        theme
                                            .collapsible_header(
                                                if self.running { "Working" } else { "Details" },
                                                open,
                                            )
                                            .hover(widgets::collapsible_header_hover),
                                    ),
                            )
                            .when(open, |el| {
                                el.child(
                                    div()
                                        .ml(px(10.0))
                                        .pl(px(12.0))
                                        .border_l_1()
                                        .border_color(theme.border)
                                        .text_size(px(12.5))
                                        .text_color(theme.text_muted)
                                        .child(if self.running {
                                            "Reading crates/ui/src/widgets.rs…"
                                        } else {
                                            "Read crates/ui/src/widgets.rs."
                                        }),
                                )
                            }),
                    )
                    .into_any_element()
            }

            "breadcrumb" => section
                .child(
                    theme
                        .breadcrumb()
                        .child(theme.breadcrumb_item("crates", false))
                        .child(theme.breadcrumb_separator())
                        .child(theme.breadcrumb_item("ui", false))
                        .child(theme.breadcrumb_separator())
                        .child(theme.breadcrumb_item("widgets.rs", true)),
                )
                .into_any_element(),

            "tag" => section
                .child(row().child(theme.tag("rust")).child(theme.tag("gpui")))
                .into_any_element(),

            "status-dot" => section
                .child(
                    row()
                        .child(widgets::status_dot(theme.success))
                        .child(widgets::status_dot(theme.warning))
                        .child(widgets::status_dot(theme.danger))
                        .child(widgets::status_dot(theme.busy)),
                )
                .into_any_element(),

            "tooltip" => section
                .child(hint(
                    &theme,
                    "Hover and hold — the label appears after 500ms.",
                ))
                .child(
                    row().child(
                        div()
                            .id("tip")
                            .tooltip(|window, cx| {
                                Tooltip::with_keystroke("Copy path", "⌘C", window, cx)
                            })
                            .child(theme.button(
                                "Hover me",
                                ButtonStyle::Ghost,
                                Some("g-tip".into()),
                            )),
                    ),
                )
                .into_any_element(),

            "hover-card" => section
                .child(hint(
                    &theme,
                    "Hoverable, unlike a tooltip: the pointer can travel into the card.",
                ))
                .child(
                    row().child(
                        div()
                            .id("hover-card")
                            .hoverable_tooltip(|window, cx| {
                                HoverCard::person(
                                    "TC",
                                    "clearloop",
                                    "Builds desktop software in Rust. Maintains bezel.",
                                    "Joined 2019 · 412 repositories",
                                    window,
                                    cx,
                                )
                            })
                            .child(theme.tag("@clearloop")),
                    ),
                )
                .into_any_element(),

            "tabs" => section
                .child(hint(&theme, "space or enter opens the focused tab."))
                .child(
                    theme.tab_bar().children(
                        ["Components", "Tokens", "Motion"]
                            .into_iter()
                            .enumerate()
                            .map(|(index, label)| {
                                pressable(
                                    focus::focusable(
                                        &theme,
                                        &self.tab_strip[index],
                                        theme.tab(label, self.tab_choice == index),
                                    ),
                                    SharedString::from(format!("tab-{index}")),
                                    cx,
                                    move |view, cx| {
                                        view.tab_choice = index;
                                        cx.notify();
                                    },
                                )
                                .into_any_element()
                            }),
                    ),
                )
                .into_any_element(),

            "split" => {
                let hint = hint(&theme, "Drag the divider; it clamps at 15% either side.");
                let muted = theme.text_muted;
                let pane = move |label: SharedString| {
                    div()
                        .h_full()
                        .p(px(12.0))
                        .text_size(px(12.5))
                        .text_color(muted)
                        .child(label)
                };
                section
                    .child(hint)
                    .child(
                        div()
                            .id("split")
                            .w(px(420.0))
                            .h(px(140.0))
                            .rounded(px(Theme::PANEL_RADIUS))
                            .border_1()
                            .border_color(theme.border)
                            .overflow_hidden()
                            .flex()
                            .flex_row()
                            .on_drag_move(cx.listener(
                                |view, event: &DragMoveEvent<SplitDrag>, _, cx| {
                                    view.split = widgets::axis_fraction(
                                        event.event.position,
                                        event.bounds,
                                        Axis::Horizontal,
                                        0.15,
                                    );
                                    view.split_dragging = true;
                                    cx.notify();
                                },
                            ))
                            // Both, because the release can land anywhere: a
                            // divider left lit reads as still grabbed.
                            .on_mouse_up(
                                gpui::MouseButton::Left,
                                cx.listener(|view, _, _, cx| {
                                    view.split_dragging = false;
                                    cx.notify();
                                }),
                            )
                            .on_mouse_up_out(
                                gpui::MouseButton::Left,
                                cx.listener(|view, _, _, cx| {
                                    view.split_dragging = false;
                                    cx.notify();
                                }),
                            )
                            .child(div().w(relative(self.split)).child(pane(SharedString::from(
                                format!("{:.0}%", self.split * 100.0),
                            ))))
                            .child(
                                theme
                                    .split_handle(Axis::Horizontal, self.split_dragging)
                                    .id("split-handle")
                                    .on_drag(SplitDrag, |_, _, _, cx| cx.new(|_| Empty)),
                            )
                            .child(div().flex_1().child(pane("drag the divider".into()))),
                    )
                    .into_any_element()
            }

            "control-bar" => {
                let glyph = |path: &'static str| {
                    let hover = theme.glass_hover();
                    ui::control_bar::bar_button(path, 30.0, theme.text_muted)
                        .id(path)
                        .hover(move |s| s.bg(hover))
                };
                let label = |copy: &'static str| {
                    div()
                        .text_size(px(12.0))
                        .text_color(theme.text_muted)
                        .child(copy)
                };
                section
                    .child(hint(
                        &theme,
                        "One bar, three jobs, two shapes. The centre is centred on \
                         the BAR rather than on what the clusters leave — five \
                         controls on the left and one on the right still put it \
                         on axis.",
                    ))
                    .child(theme.field_label("Transport — Shape::Pill"))
                    .child(ui::control_bar::control_bar(
                        &theme,
                        ControlBarShape::Pill,
                        vec![
                            glyph(icons::SHUFFLE).into_any_element(),
                            glyph(icons::SKIP_PREVIOUS).into_any_element(),
                            glyph(icons::PLAY_BOLD).into_any_element(),
                            glyph(icons::SKIP_NEXT).into_any_element(),
                            glyph(icons::REPEAT).into_any_element(),
                        ],
                        Some(label("Grain").into_any_element()),
                        vec![glyph(icons::VOLUME_LOUD).into_any_element()],
                    ))
                    // Rounded, not a stadium: a composer is not a media control,
                    // and the stadium reads as one.
                    .child(theme.field_label("Composer — Shape::Rounded"))
                    .child(ui::control_bar::control_bar(
                        &theme,
                        ControlBarShape::Rounded,
                        vec![glyph(icons::PLUS).into_any_element()],
                        Some(label("Ask anything…").into_any_element()),
                        vec![
                            glyph(icons::MICROPHONE).into_any_element(),
                            glyph(icons::ARROW_UP).into_any_element(),
                        ],
                    ))
                    .child(theme.field_label("Floating over content"))
                    // The same striped band the materials page uses, and the only
                    // place either shape's blur can be caught disagreeing with
                    // its border.
                    .child(
                        div()
                            .relative()
                            .h(px(130.0))
                            .rounded(px(Theme::PANEL_RADIUS))
                            .overflow_hidden()
                            .child(div().absolute().inset_0().flex().flex_row().children(
                                (0..14).map(|i| {
                                    div().flex_1().h_full().bg(if i % 2 == 0 {
                                        theme.accent
                                    } else {
                                        theme.warning
                                    })
                                }),
                            ))
                            .child(
                                div()
                                    .absolute()
                                    .bottom(px(16.0))
                                    .left_0()
                                    .right_0()
                                    .flex()
                                    .justify_center()
                                    .child(ui::control_bar::control_bar(
                                        &theme,
                                        ControlBarShape::Pill,
                                        vec![
                                            glyph(icons::SIDEBAR_MINIMALISTIC_LEFT)
                                                .into_any_element(),
                                            glyph(icons::MAGNIFER).into_any_element(),
                                        ],
                                        None,
                                        vec![
                                            glyph(icons::TUNING).into_any_element(),
                                            glyph(icons::SETTINGS_MINIMALISTIC).into_any_element(),
                                        ],
                                    )),
                            ),
                    )
                    .into_any_element()
            }

            "menu" => section
                .child(
                    popover::popover_card(&theme).w(px(240.0)).children([
                        popover::menu_heading(&theme, "Section").into_any_element(),
                        popover::menu_row(&theme, false, "m-one")
                            .child("First item")
                            .into_any_element(),
                        popover::menu_row(&theme, true, "m-two")
                            .child("Active item")
                            .into_any_element(),
                        popover::divider().into_any_element(),
                        popover::menu_row(&theme, false, "m-three")
                            .child("Third item")
                            .into_any_element(),
                    ]),
                )
                .into_any_element(),

            "group-box" => section
                .child(
                    theme
                        .group_box()
                        .child(
                            theme
                                .card_row(true)
                                .hover(widgets::card_row_hover)
                                .child(theme.row_tile(icons::MONITOR))
                                .child(theme.row_title("First row")),
                        )
                        .child(
                            theme
                                .card_row(false)
                                .hover(widgets::card_row_hover)
                                .child(theme.row_tile(icons::FOLDER))
                                .child(theme.row_title("Second row")),
                        ),
                )
                .into_any_element(),

            "empty-state" => section
                .child(theme.group_box().child(theme.empty_state(
                    icons::FOLDER,
                    "No repositories",
                    "Open a folder to get started.",
                )))
                .into_any_element(),

            "loaders" => section
                .child(hint(
                    &theme,
                    "The four orbs are bezel's own. Everything below them is a grid \
                     of cells; the orbs are circles, because circles are the whole \
                     vocabulary gpui gives at this rev — no rotation, no conic \
                     gradient, no blur filter.",
                ))
                .child(
                    row().gap(px(24.0)).children(
                        [
                            (loaders::Orb::Cluster, "cluster"),
                            (loaders::Orb::Ring, "ring"),
                            (loaders::Orb::Converge, "converge"),
                            (loaders::Orb::Bloom, "bloom"),
                        ]
                        .map(|(shape, label)| {
                            stack()
                                .items_center()
                                .gap(px(10.0))
                                .child(loaders::orb(shape, label, 44.0, &theme, view, cx))
                                .child(
                                    div()
                                        .text_size(px(10.5))
                                        .font_family(theme.font_mono.clone())
                                        .text_color(theme.text_faint)
                                        .child(label),
                                )
                        }),
                    ),
                )
                .child(hint(&theme, "And the older three:"))
                .child(
                    row()
                        .child(loaders::pulse_loader("g-pulse", &theme, 8.0, view, cx))
                        .child(loaders::gradient_spinner("g-spin", &theme, 5.0, view, cx))
                        .child(loaders::mini_gradient_spinner("g-mini", 2.5, view, cx))
                        .child(loaders::loading_word(&theme)),
                )
                .into_any_element(),

            "palette" => section
                .child(
                    row()
                        .child(popover::key_hint_text(&theme, "⌘K", "open palette"))
                        .when_some(self.last_command.clone(), |r, cmd| {
                            r.child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(theme.text_muted)
                                    .child(SharedString::from(format!("ran: {cmd}"))),
                            )
                        }),
                )
                .into_any_element(),

            "sheet" => section
                .child(hint(
                    &theme,
                    "A dialog pinned to an edge; the scrim dismisses it.",
                ))
                .child(
                    row().child(
                        div()
                            .id("open-sheet")
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.sheet.open(());
                                cx.notify();
                            }))
                            .child(theme.button(
                                "Open sheet",
                                ButtonStyle::Ghost,
                                Some("g-sheet".into()),
                            )),
                    ),
                )
                .into_any_element(),

            "context-menu" => section
                .child(hint(&theme, "Right-click anywhere in this window."))
                .child(
                    div()
                        .h(px(120.0))
                        .w_full()
                        .rounded(px(Theme::PANEL_RADIUS))
                        .border_1()
                        .border_color(theme.border)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(12.5))
                        .text_color(theme.text_faint)
                        .child("right-click"),
                )
                .into_any_element(),

            "dialog" => section
                .child(hint(&theme, "A centred card over a dim scrim."))
                .child(
                    row().child(
                        div()
                            .id("open-dialog")
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.dialog.open(());
                                cx.notify();
                            }))
                            .child(theme.button(
                                "Open dialog",
                                ButtonStyle::Ghost,
                                Some("g-dialog".into()),
                            )),
                    ),
                )
                .into_any_element(),

            // Exercises the fork's backdrop-blur primitive: the card blurs the
            // striped band painted behind it.
            "material" => {
                section
                    .child(
                        div()
                            .relative()
                            .w(px(420.0))
                            .h(px(150.0))
                            .child(div().absolute().inset_0().flex().flex_row().children(
                                (0..14).map(|i| {
                                    div().w(px(30.0)).h_full().bg(if i % 2 == 0 {
                                        theme.accent
                                    } else {
                                        theme.warning
                                    })
                                }),
                            ))
                            .child(div().absolute().top(px(28.0)).left(px(60.0)).child(
                                ui::material::material(
                                    12.0,
                                    ui::material::MENU_BLUR,
                                    popover::popover_card(&theme).w(px(220.0)).child(
                                        popover::menu_row(&theme, false, "mat-a").child("Blurred"),
                                    ),
                                ),
                            )),
                    )
                    .into_any_element()
            }

            "alerts" => section
                .child(theme.error_strip("Something went wrong."))
                .child(theme.warning_strip("Heads up, check this."))
                .into_any_element(),

            "step-row" => {
                let card = |index: usize, first: bool| {
                    let Step {
                        icon,
                        title,
                        detail,
                        meta,
                        failed,
                        output,
                    } = STEPS[index];
                    let open = self.step_open[index];
                    div()
                        .when(!first, |el| el.border_t_1().border_color(theme.border))
                        .child(
                            theme
                                .step_row(
                                    icon,
                                    title,
                                    Some(SharedString::from(detail)),
                                    Some(SharedString::from(meta)),
                                    failed,
                                    output.map(|_| open),
                                )
                                .hover(widgets::step_row_hover)
                                .id(SharedString::from(format!("step-{index}")))
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.step_open[index] = !view.step_open[index];
                                    cx.notify();
                                })),
                        )
                        .when_some(output.filter(|_| open), |el, output| {
                            el.child(theme.step_output(
                                SharedString::from(format!("step-out-{index}")),
                                output,
                            ))
                        })
                };

                section
                    .child(hint(
                        &theme,
                        "An operation with an outcome: press a row to see what it \
                         printed. A step with no output has no chevron — a \
                         disclosure onto nothing is worse than none.",
                    ))
                    .child(
                        // Standalone: one step in its own box.
                        div()
                            .w(px(420.0))
                            .rounded(px(Theme::PANEL_RADIUS))
                            .border_1()
                            .border_color(theme.border)
                            .overflow_hidden()
                            .child(card(0, true)),
                    )
                    .child(hint(
                        &theme,
                        "Or as a run: the same rows, borderless, in one box that \
                         owns the hairlines between them.",
                    ))
                    .child(
                        div()
                            .w(px(420.0))
                            .rounded(px(Theme::PANEL_RADIUS))
                            .border_1()
                            .border_color(theme.border)
                            .overflow_hidden()
                            .child(card(1, true))
                            .child(card(2, false)),
                    )
                    .into_any_element()
            }

            "skeleton" => section
                .child(popover::redacted_rows("g-redacted", &theme, 3, view, cx))
                .into_any_element(),

            "textarea" => section
                .child(hint(
                    &theme,
                    "The same TextField under a different Shape — enter breaks a line, \
                     up/down and ctrl-p/ctrl-n walk rows keeping their column, and \
                     ctrl-a/ctrl-e go to the ends of the logical line rather than \
                     stopping at a wrap.",
                ))
                .child(shape_demo(&theme, "Shape::Rows(4)", self.notes.clone()))
                .child(shape_demo(
                    &theme,
                    "Shape::Grow { min: 2, max: 6 }",
                    self.composer.clone(),
                ))
                .child(hint(
                    &theme,
                    "Past the last row it scrolls: the caret is kept in view as you \
                     type or move, and the wheel scrolls away from it without being \
                     dragged back.",
                ))
                .into_any_element(),

            // ---- Not built yet -----------------------------------------------
            "date-picker" => section
                .child(hint(
                    &theme,
                    "enter opens it. The arrows walk days and weeks — off the end \
                     of a month and the grid follows — pageup and pagedown page \
                     months, enter chooses, escape dismisses.",
                ))
                .child(div().w(px(220.0)).child(self.date.clone()))
                .child(
                    div()
                        .text_size(px(12.5))
                        .text_color(theme.text_muted)
                        .child(SharedString::from(match self.date.read(cx).selection() {
                            Some(date) => format!("chosen: {date}"),
                            None => "nothing chosen".to_string(),
                        })),
                )
                .into_any_element(),

            "menubar" => section
                .child(hint(
                    &theme,
                    "Open one, then slide across the others — a bar with a menu \
                     down switches on hover, with no second click. The arrows \
                     walk rows and cross between menus; the greyed rows cannot \
                     be landed on at all.",
                ))
                .child(self.menubar.clone())
                .child(
                    div()
                        .text_size(px(12.5))
                        .text_color(theme.text_muted)
                        .child(SharedString::from(match &self.last_menu_item {
                            Some(label) => format!("chose: {label}"),
                            None => "nothing chosen".to_string(),
                        })),
                )
                .into_any_element(),

            "pagination" => {
                section
                    .child(hint(
                        &theme,
                        "For data that arrives in pages — a result set the client \
                     cannot hold — and not for lists that are merely long: those \
                     are the scroll area and the virtualized list. Walk to either \
                     end and the run of pages keeps its width.",
                    ))
                    .child(
                        pagination::pagination()
                            .child(
                                div()
                                    .id("page-prev")
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.go_to_page(view.page.saturating_sub(1), cx)
                                    }))
                                    .child(pagination::step(
                                        &theme,
                                        icons::ALT_ARROW_LEFT,
                                        self.page > 1,
                                    )),
                            )
                            .children(
                                pagination::window(self.page, RESULT_PAGES, 2)
                                    .into_iter()
                                    .enumerate()
                                    .map(|(slot, entry)| match entry {
                                        pagination::Slot::Gap => {
                                            pagination::ellipsis(&theme).into_any_element()
                                        }
                                        pagination::Slot::Page(page) => {
                                            pagination::page_button(&theme, page, page == self.page)
                                                .id(SharedString::from(format!("page-{slot}")))
                                                .on_click(cx.listener(move |view, _, _, cx| {
                                                    view.go_to_page(page, cx)
                                                }))
                                                .into_any_element()
                                        }
                                    }),
                            )
                            .child(
                                div()
                                    .id("page-next")
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.go_to_page(view.page + 1, cx)
                                    }))
                                    .child(pagination::step(
                                        &theme,
                                        icons::ALT_ARROW_RIGHT,
                                        self.page < RESULT_PAGES,
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(12.5))
                            .text_color(theme.text_muted)
                            .child(SharedString::from(format!(
                                "page {} of {RESULT_PAGES}",
                                self.page
                            ))),
                    )
                    .into_any_element()
            }

            "scroll-area" => section
                .child(hint(
                    &theme,
                    "Drag the thumb, or use the wheel over it — the bar overlays \
                     the content rather than taking a gutter, so it never reflows \
                     what it sits on. The rail and this pane have one too.",
                ))
                .child(
                    div()
                        .relative()
                        .h(px(220.0))
                        .w_full()
                        .rounded(px(Theme::PANEL_RADIUS))
                        .border_1()
                        .border_color(theme.border)
                        .overflow_hidden()
                        .child(
                            div()
                                .id("scroll-demo")
                                .size_full()
                                .overflow_y_scroll()
                                .track_scroll(&self.demo_scroll)
                                .child(div().p(px(14.0)).flex().flex_col().gap(px(8.0)).children(
                                    (1..=30).map(|line| {
                                        div()
                                            .text_size(px(12.5))
                                            .text_color(theme.text_muted)
                                            .child(SharedString::from(format!("Line {line}")))
                                    }),
                                )),
                        )
                        .child(scroll::scrollbar(
                            "scroll-demo-bar",
                            &self.demo_scroll,
                            &self.demo_bar,
                        )),
                )
                .into_any_element(),

            "follow" => section
                .child(hint(
                    &theme,
                    "Append a line and the box stays on the newest one. Scroll up \
                     and it lets go — scroll back to the bottom and it takes over \
                     again. Neither is an event it subscribes to: the overflow \
                     changing is what tells appended content apart from you.",
                ))
                .child(
                    row()
                        .child(
                            div()
                                .id("follow-append")
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.log_lines += 1;
                                    cx.notify();
                                }))
                                .child(theme.button(
                                    "Append a line",
                                    ButtonStyle::Ghost,
                                    Some("g-follow-add".into()),
                                )),
                        )
                        .child(
                            div()
                                .id("follow-jump")
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.log_follow.follow();
                                    cx.notify();
                                }))
                                .child(theme.button(
                                    "Jump to latest",
                                    ButtonStyle::Ghost,
                                    Some("g-follow-pin".into()),
                                )),
                        )
                        // The state, on the page — the same trick the virtualized
                        // list uses for its built count. A behaviour you can only
                        // infer is a behaviour nobody can check.
                        .child(
                            div()
                                .text_size(px(12.0))
                                .font_family(theme.font_mono.clone())
                                .text_color(if self.log_follow.following() {
                                    theme.success
                                } else {
                                    theme.text_faint
                                })
                                .child(SharedString::from(format!(
                                    "following: {}",
                                    self.log_follow.following()
                                ))),
                        ),
                )
                .child(
                    div()
                        .relative()
                        .h(px(180.0))
                        .w_full()
                        .rounded(px(Theme::PANEL_RADIUS))
                        .border_1()
                        .border_color(theme.border)
                        .overflow_hidden()
                        .child(
                            div()
                                .id("follow-demo")
                                .size_full()
                                .overflow_y_scroll()
                                .track_scroll(&self.log_scroll)
                                .child(div().p(px(14.0)).flex().flex_col().gap(px(6.0)).children(
                                    (1..=self.log_lines).map(|line| {
                                        div()
                                            .text_size(px(12.0))
                                            .font_family(theme.font_mono.clone())
                                            .text_color(theme.text_muted)
                                            .child(SharedString::from(format!(
                                                "[{line:04}] token stream line {line}"
                                            )))
                                    }),
                                )),
                        )
                        .child(scroll::follow(&self.log_scroll, &self.log_follow))
                        .child(scroll::scrollbar(
                            "follow-demo-bar",
                            &self.log_scroll,
                            &self.log_bar,
                        )),
                )
                .into_any_element(),

            "table" => {
                let columns = table_columns();
                let mut rows = TABLE_ROWS;
                if let Some(sort) = self.table_sort {
                    // bezel never sees the rows: it says what the click meant
                    // and paints the arrow, and the sorting happens here.
                    rows.sort_by(|left, right| {
                        let order = match sort.column {
                            0 => left.0.cmp(right.0),
                            1 => left.1.cmp(right.1),
                            _ => left.2.cmp(&right.2),
                        };
                        if sort.ascending {
                            order
                        } else {
                            order.reverse()
                        }
                    });
                }
                section
                    .child(hint(
                        &theme,
                        "Click a heading to sort, and again to reverse it. The \
                         header sits outside the scroll container, so it stays \
                         put while the body moves under it.",
                    ))
                    .child(
                        table::table(&theme)
                            .child(
                                table::header(&theme).children(columns.iter().enumerate().map(
                                    |(index, column)| {
                                        let sorted = self
                                            .table_sort
                                            .filter(|sort| sort.column == index)
                                            .map(|sort| sort.ascending);
                                        table::header_cell(&theme, column, sorted)
                                            .id(SharedString::from(format!("column-{index}")))
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                view.sort_table(index, cx)
                                            }))
                                            .into_any_element()
                                    },
                                )),
                            )
                            .child(
                                div()
                                    .relative()
                                    .h(px(150.0))
                                    .child(
                                        div()
                                            .id("table-body")
                                            .size_full()
                                            .overflow_y_scroll()
                                            .track_scroll(&self.table_scroll)
                                            .children(rows.iter().enumerate().map(
                                                |(index, (name, kind, size))| {
                                                    table::row(
                                                        &theme,
                                                        &columns,
                                                        index == 0,
                                                        false,
                                                        vec![
                                                            SharedString::from(*name)
                                                                .into_any_element(),
                                                            div()
                                                                .text_color(theme.text_muted)
                                                                .child(SharedString::from(*kind))
                                                                .into_any_element(),
                                                            div()
                                                                .font_family(
                                                                    theme.font_mono.clone(),
                                                                )
                                                                .text_color(theme.text_muted)
                                                                .child(SharedString::from(
                                                                    format_size(*size),
                                                                ))
                                                                .into_any_element(),
                                                        ],
                                                    )
                                                },
                                            )),
                                    )
                                    .child(scroll::scrollbar(
                                        "table-bar",
                                        &self.table_scroll,
                                        &self.table_bar,
                                    )),
                            ),
                    )
                    .into_any_element()
            }

            "tree" => {
                let rows = self.tree_rows();
                section
                    .child(hint(
                        &theme,
                        "Click a folder to open it, a file to choose it. The \
                         arrows walk the same rows: right opens a folder or \
                         steps into it, left closes it or leaves for its parent.",
                    ))
                    .child(
                        div()
                            .key_context(tree::KEY_CONTEXT)
                            .track_focus(&self.tree_focus)
                            .on_action(cx.listener(|view, _: &tree::SelectPrevious, _, cx| {
                                view.tree_step(Direction::Up, cx)
                            }))
                            .on_action(cx.listener(|view, _: &tree::SelectNext, _, cx| {
                                view.tree_step(Direction::Down, cx)
                            }))
                            .on_action(cx.listener(|view, _: &tree::Collapse, _, cx| {
                                view.tree_step(Direction::Left, cx)
                            }))
                            .on_action(cx.listener(|view, _: &tree::Expand, _, cx| {
                                view.tree_step(Direction::Right, cx)
                            }))
                            .relative()
                            .h(px(200.0))
                            .w_full()
                            .rounded(px(Theme::PANEL_RADIUS))
                            .border_1()
                            .border_color(theme.border)
                            .overflow_hidden()
                            .child(
                                div()
                                    .id("tree-body")
                                    .size_full()
                                    .overflow_y_scroll()
                                    .track_scroll(&self.tree_scroll)
                                    .child(tree::tree().p(px(6.0)).children(
                                        rows.iter().enumerate().map(|(index, entry)| {
                                            tree::tree_row(
                                                &theme,
                                                &entry.row,
                                                self.tree_selected.as_deref() == Some(&entry.path),
                                                index == self.tree_cursor,
                                            )
                                            .id(SharedString::from(format!("tree-{index}")))
                                            .on_click(cx.listener(move |view, _, window, cx| {
                                                view.tree_click(index, window, cx)
                                            }))
                                            .child(SharedString::from(entry.label))
                                        }),
                                    )),
                            )
                            .child(scroll::scrollbar(
                                "tree-bar",
                                &self.tree_scroll,
                                &self.tree_bar,
                            )),
                    )
                    .when_some(self.tree_selected.clone(), |page, path| {
                        page.child(
                            div()
                                .text_size(px(12.5))
                                .text_color(theme.text_muted)
                                .child(SharedString::from(format!("chosen: {path}"))),
                        )
                    })
                    .into_any_element()
            }

            "virtual-list" => {
                let built = self.rows_built.clone();
                let muted = theme.text_muted;
                let mono = theme.font_mono.clone();
                section
                    .child(hint(
                        &theme,
                        "Ten thousand rows. The count below is how many of them \
                         the list actually built for the frame you are looking \
                         at — scroll it and it stays about the same.",
                    ))
                    .child(
                        div()
                            .relative()
                            .h(px(220.0))
                            .w_full()
                            .rounded(px(Theme::PANEL_RADIUS))
                            .border_1()
                            .border_color(theme.border)
                            .overflow_hidden()
                            .child(list::virtual_list(
                                "virtual-rows",
                                VIRTUAL_ROWS,
                                px(26.0),
                                &self.rows_scroll,
                                move |range, _, _| {
                                    built.set(range.len());
                                    range
                                        .map(|index| {
                                            div()
                                                .flex()
                                                .flex_row()
                                                .items_center()
                                                .gap(px(10.0))
                                                .px(px(12.0))
                                                .text_size(px(12.5))
                                                .child(
                                                    div()
                                                        .w(px(56.0))
                                                        .flex_none()
                                                        .font_family(mono.clone())
                                                        .text_color(muted)
                                                        .child(SharedString::from(format!(
                                                            "{index:05}"
                                                        ))),
                                                )
                                                .child(SharedString::from(format!("Row {index}")))
                                        })
                                        .collect::<Vec<_>>()
                                },
                            ))
                            .child(scroll::scrollbar(
                                "virtual-bar",
                                &list::scroll_handle(&self.rows_scroll),
                                &self.rows_bar,
                            )),
                    )
                    .child(
                        div()
                            .text_size(px(12.5))
                            .font_family(theme.font_mono.clone())
                            .text_color(theme.text_muted)
                            .child(SharedString::from(format!(
                                "{VIRTUAL_ROWS} rows · {} built this frame",
                                self.rows_built.get()
                            ))),
                    )
                    .into_any_element()
            }

            // ---- Patterns ----------------------------------------------------
            "agent-activity" => self.activity.clone().into_any_element(),
            "agent-tools" => self.tool_calls.clone().into_any_element(),
            "agent-composer" => self.agent_composer.clone().into_any_element(),
            "agent-transcript" => self.transcript.clone().into_any_element(),
            "agent-diff" => self.diff.clone().into_any_element(),
            "document" => self.document.clone().into_any_element(),
            #[cfg(not(target_family = "wasm"))]
            "agent-terminal" => self.terminal.clone().into_any_element(),
            "agent-orbs" => self.orbs.clone().into_any_element(),
            "syntax" => self.syntax.clone().into_any_element(),
            "agent-avatar" => self.avatar.clone().into_any_element(),

            _ => div().into_any_element(),
        }
    }
}

/// The vertical rhythm every page body uses.
fn stack() -> gpui::Div {
    div().flex().flex_col().gap(px(12.0))
}

/// The text sizes the library actually paints, gathered from the source rather
/// than invented as a scale — bezel has no formal type ramp, and pretending it
/// does would document a thing that is not there.
const TYPE_SCALE: &[f32] = &[
    10.0, 10.5, 11.0, 11.5, 12.0, 12.5, 13.0, 13.5, 14.0, 15.0, 16.0,
];

/// Every named spec in `motion`.
const MOTION_CATALOG: &[(&str, motion::MotionSpec)] = &[
    ("FADE_IN", motion::FADE_IN),
    ("FADE_QUICK", motion::FADE_QUICK),
    ("MENU_IN", motion::MENU_IN),
    ("MENU_OUT", motion::MENU_OUT),
    ("DIALOG_IN", motion::DIALOG_IN),
    ("SPLASH_OUT", motion::SPLASH_OUT),
    ("RESIZE", motion::RESIZE),
    ("TAB_SLIDE", motion::TAB_SLIDE),
    ("COLLAPSE", motion::COLLAPSE),
    ("CHEVRON", motion::CHEVRON),
    ("SCROLL_GLIDE", motion::SCROLL_GLIDE),
    ("HOVER_FADE", motion::HOVER_FADE),
    ("PULSE", motion::PULSE),
    ("GRADIENT_SPIN", motion::GRADIENT_SPIN),
];

/// The colour tokens, by role. Hand-listed because `Theme` is a plain struct —
/// there is no reflection, and a token that never reaches this list is a token
/// nobody can find.
fn color_groups(theme: &Theme) -> Vec<(&'static str, Vec<(&'static str, gpui::Hsla)>)> {
    vec![
        (
            "Surfaces",
            vec![
                ("bg", theme.bg),
                ("surface", theme.surface),
                ("surface_raised", theme.surface_raised),
                ("surface_card", theme.surface_card),
                ("surface_dialog", theme.surface_dialog),
                ("surface_overlay", theme.surface_overlay),
                ("band", theme.band),
                ("input_bg", theme.input_bg),
            ],
        ),
        (
            "Text",
            vec![
                ("text", theme.text),
                ("text_muted", theme.text_muted),
                ("text_faint", theme.text_faint),
                ("text_dim", theme.text_dim),
                ("on_solid", theme.on_solid),
                ("on_accent", theme.on_accent),
            ],
        ),
        (
            "Lines & fills",
            vec![
                ("border", theme.border),
                ("border_strong", theme.border_strong),
                ("element_hover", theme.element_hover),
                ("element_active", theme.element_active),
                ("selection", theme.selection),
                ("caret", theme.caret),
                ("solid", theme.solid),
            ],
        ),
        (
            "Accent & status",
            vec![
                ("accent", theme.accent),
                ("accent_strong", theme.accent_strong),
                ("success", theme.success),
                ("success_muted", theme.success_muted),
                ("warning", theme.warning),
                ("warning_muted", theme.warning_muted),
                ("danger", theme.danger),
                ("danger_muted", theme.danger_muted),
                ("danger_strong", theme.danger_strong),
                ("busy", theme.busy),
            ],
        ),
        (
            "Code & diff",
            vec![
                ("code_text", theme.code_text),
                ("code_wash", theme.code_wash),
                ("diff_add", theme.diff_add),
                ("diff_del", theme.diff_del),
                ("diff_hunk_bg", theme.diff_hunk_bg),
            ],
        ),
    ]
}

/// One colour chip: the paint over the page background, its token name, and the
/// contrast it lands at — the number that decides whether text on it is legible.
fn swatch(theme: &Theme, name: &'static str, color: gpui::Hsla) -> gpui::Div {
    div()
        .w(px(124.0))
        .flex()
        .flex_col()
        .gap(px(5.0))
        .child(
            div()
                .h(px(44.0))
                .w_full()
                .rounded(px(Theme::CONTROL_RADIUS))
                .border_1()
                .border_color(theme.border)
                .bg(color),
        )
        .child(
            div()
                .text_size(px(10.5))
                .font_family(theme.font_mono.clone())
                .text_color(theme.text_muted)
                .child(SharedString::from(name)),
        )
        .child(
            div()
                .text_size(px(10.0))
                .text_color(theme.text_faint)
                .child(SharedString::from(format!(
                    "{:.1}:1",
                    theme::contrast_ratio(color, theme.bg)
                ))),
        )
}

/// A constant drawn at its own size, so a number reads as a distance.
fn measure(theme: &Theme, name: &'static str, value: f32) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(12.0))
        .child(
            div()
                .w(px(170.0))
                .flex_none()
                .text_size(px(11.5))
                .font_family(theme.font_mono.clone())
                .text_color(theme.text_muted)
                .child(SharedString::from(name)),
        )
        .child(
            div()
                .h(px(10.0))
                .w(px(value))
                .rounded(px(2.0))
                .bg(theme.accent),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(theme.text_faint)
                .child(SharedString::from(format!("{value}"))),
        )
}

/// An easing curve as a bar chart of its own output. gpui has no path drawing
/// at this rev, and sampling the real function beats drawing an approximation
/// of it.
fn curve_plot(theme: &Theme, name: &str, at: impl Fn(f32) -> f32) -> gpui::Div {
    const SAMPLES: usize = 28;
    const HEIGHT: f32 = 40.0;
    let plot = div()
        .flex()
        .flex_row()
        .items_end()
        .gap(px(1.0))
        .h(px(HEIGHT))
        .children((0..SAMPLES).map(|i| {
            let t = i as f32 / (SAMPLES - 1) as f32;
            div()
                .w(px(3.0))
                .h(px((at(t).clamp(0.0, 1.0) * HEIGHT).max(1.0)))
                .rounded(px(1.0))
                .bg(theme.accent)
        }));
    if name.is_empty() {
        plot
    } else {
        div().flex().flex_col().gap(px(6.0)).child(plot).child(
            div()
                .text_size(px(11.0))
                .font_family(theme.font_mono.clone())
                .text_color(theme.text_faint)
                .child(SharedString::from(name.to_string())),
        )
    }
}

/// A font family shown in itself, named beside it.
fn type_row(theme: &Theme, family: SharedString, name: &'static str) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_baseline()
        .gap(px(12.0))
        .child(
            div()
                .w(px(90.0))
                .flex_none()
                .text_size(px(11.0))
                .font_family(theme.font_mono.clone())
                .text_color(theme.text_faint)
                .child(SharedString::from(name)),
        )
        .child(
            div()
                .text_size(px(15.0))
                .font_family(family.clone())
                .child(family),
        )
}

/// One muted line telling you how to try a component whose whole behaviour is
/// an interaction — the page would otherwise look like a dead button.
fn hint(theme: &Theme, copy: &str) -> gpui::Div {
    div()
        .text_size(px(12.5))
        .text_color(theme.text_muted)
        .child(SharedString::from(copy.to_string()))
}

/// One field captioned with the [`Shape`] that produced it, so the numbers on
/// the page read as this example's arguments rather than as library defaults.
fn shape_demo(theme: &Theme, shape: &'static str, field: Entity<TextField>) -> gpui::Div {
    stack()
        .gap(px(6.0))
        .child(
            div()
                .text_size(px(11.0))
                .font_family(theme.font_mono.clone())
                .text_color(theme.text_faint)
                .child(SharedString::from(shape)),
        )
        .child(field)
}

/// The page for a [`planned`] component: why it is not here, and what the work
/// actually is. Numbered rather than bulleted — the entries are the order the
/// work happens in, not a set of features.
///
/// `work` is empty wherever nothing has been designed yet. That is the honest
/// answer, and an empty list is itself the measurement.
///
/// Unused for the same reason [`planned`] is: nothing in the catalog is unbuilt
/// at the moment. Kept for the next thing that is.
#[allow(dead_code)]
fn todo(theme: &Theme, status: &str, summary: &str, work: &[&'static str]) -> AnyElement {
    let amber = theme.warning;
    stack()
        .child(
            div()
                .self_start()
                .px(px(7.0))
                .py(px(2.0))
                .rounded(px(5.0))
                .border_1()
                .border_color(amber.opacity(0.2))
                .bg(amber.opacity(0.06))
                .text_size(px(10.0))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(theme.warning_muted.opacity(0.9))
                .child(SharedString::from(popover::tracked_upper(status))),
        )
        .child(
            div()
                .text_size(px(13.0))
                .text_color(theme.text_muted)
                .child(SharedString::from(summary.to_string())),
        )
        .when(!work.is_empty(), |page| {
            page.child(
                theme
                    .group_box()
                    .children(work.iter().enumerate().map(|(index, step)| {
                        theme
                            .card_row(index == 0)
                            .hover(widgets::card_row_hover)
                            .items_start()
                            .child(
                                div()
                                    .flex_none()
                                    .w(px(12.0))
                                    .text_size(px(12.0))
                                    .font_family(theme.font_mono.clone())
                                    .text_color(theme.text_faint)
                                    .child(SharedString::from(format!("{}", index + 1))),
                            )
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .text_color(theme.text_muted)
                                    .child(SharedString::from(*step)),
                            )
                            .into_any_element()
                    })),
            )
        })
        .into_any_element()
}

/// Wire a focusable control to click *and* to `enter`/`space`, from one closure.
///
/// [`focus::Activate`] is dispatched rather than folded into `on_click` because
/// only the caller knows what a press means — which makes two call sites per
/// control, and two call sites are where a keyboard affordance quietly starts
/// doing something else than the mouse. Taking the behaviour once removes the
/// chance.
fn pressable<T: 'static>(
    el: gpui::Div,
    id: impl Into<gpui::ElementId>,
    cx: &Context<T>,
    press: impl Fn(&mut T, &mut Context<T>) + Clone + 'static,
) -> gpui::Stateful<gpui::Div> {
    let by_key = press.clone();
    el.id(id)
        .on_click(cx.listener(move |view, _, _, cx| press(view, cx)))
        .on_action(cx.listener(move |view, _: &focus::Activate, _, cx| by_key(view, cx)))
}

/// Bytes in the shortest unit that keeps them under four digits — the kind of
/// formatting a right-aligned column exists for.
fn format_size(bytes: u32) -> String {
    match bytes {
        0..1_000 => format!("{bytes} B"),
        1_000..1_000_000 => format!("{:.1} kB", bytes as f32 / 1_000.0),
        _ => format!("{:.1} MB", bytes as f32 / 1_000_000.0),
    }
}

/// Today, locally — the one thing [`Calendar`] asks its host for.
///
/// This is the boundary conversion, and the reason it is worth showing: bezel
/// carries no clock and no chrono, so the app that has both hands over three
/// numbers and keeps its date library to itself.
fn today() -> Date {
    use chrono::Datelike as _;
    let now = chrono::Local::now().date_naive();
    Date::new(now.year(), now.month() as u8, now.day() as u8).expect("chrono deals in real dates")
}

fn row() -> gpui::Div {
    div().flex().flex_row().items_center().gap(px(12.0))
}

fn column() -> gpui::Div {
    div().w(px(420.0)).flex().flex_col().gap(px(28.0))
}

impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();

        // Rail on the left, one component in the pane — the set is long enough
        // that a single scroll of everything reads as a wall.
        let section =
            section_at(self.selected[self.tab]).unwrap_or(&TABS[self.tab].groups[0].sections[0]);
        let body = self.section_body(section.key, cx);
        let pane = div().relative().flex_1().min_h_0().map(|pane| {
            if TABS[self.tab].full_bleed {
                // A pattern is a screen: it takes the pane whole and
                // scrolls its own parts, so neither the fixed column nor
                // the pane's own scrollbar applies to it.
                pane.child(div().size_full().p(px(24.0)).child(body))
            } else {
                pane.child(
                    div()
                        .id("gallery-pane")
                        .size_full()
                        .overflow_y_scroll()
                        .track_scroll(&self.pane_scroll)
                        // The column width components are designed for;
                        // several are `w_full` and would otherwise stretch
                        // to the whole pane.
                        .child(div().p(px(32.0)).child(column().child(body))),
                )
                .child(scroll::scrollbar(
                    "pane-bar",
                    &self.pane_scroll,
                    &self.pane_bar,
                ))
            }
        });
        let content = if self.embedded {
            // The page around the iframe is already the nav, the rail and the
            // header — repeating them inside it would be the same chrome twice.
            div().flex().flex_col().size_full().child(pane)
        } else {
            div()
                .flex()
                .flex_col()
                .size_full()
                .child(self.nav(&theme, cx))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_1()
                        .min_h_0()
                        .child(self.rail(&theme, cx))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .h_full()
                                .flex()
                                .flex_col()
                                .child(self.header(section, &theme))
                                .child(pane),
                        ),
                )
        };

        // A hover fade is a colour computed at paint time, not an animation
        // element that drives itself: `hover_listener` marks the window dirty
        // once when the pointer crosses, and everything after that frame is the
        // host's to ask for. Without this the blend paints its first frame — at
        // rest — and then freezes until something unrelated repaints, which
        // reads as a wash that sticks and then jumps. It also ticks the fade
        // table, which is what evicts entries for elements that have gone away.
        if motion::hover_fades_active() {
            window.request_animation_frame();
        }

        // Traversal goes on the root so `tab` works wherever focus happens to
        // be, rather than only inside whatever claimed it.
        focus::traversal(div())
            .id("gallery-scroll")
            .key_context("Gallery")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::open_palette))
            .on_action(cx.listener(Self::toggle_inspector))
            .on_action(cx.listener(Self::toggle_full_screen))
            .on_mouse_down(
                gpui::MouseButton::Right,
                cx.listener(|view, event: &gpui::MouseDownEvent, _, cx| {
                    view.context_menu.open(event.position);
                    cx.notify();
                }),
            )
            .size_full()
            .bg(theme.bg)
            .font_family(theme.font_sans.clone())
            .text_color(theme.text)
            .text_size(px(14.0))
            .child(content)
            .when_some(
                self.context_menu
                    .get()
                    .copied()
                    .map(|position| (position, self.context_menu.closing_since())),
                |root, (position, closing)| {
                    root.child(popover::menu_at(
                        "gallery-context",
                        position,
                        popover::popover_card(&theme)
                            .w(px(180.0))
                            .children(["Cut", "Copy", "Paste"].iter().enumerate().map(
                                |(index, label)| {
                                    popover::menu_row(
                                        &theme,
                                        false,
                                        SharedString::from(format!("ctx-{index}")),
                                    )
                                    .id(SharedString::from(format!("ctx-item-{index}")))
                                    .on_click(
                                        cx.listener(|view, _, _, cx| view.close_context_menu(cx)),
                                    )
                                    .child(*label)
                                    .into_any_element()
                                },
                            ))
                            // Dismissal is the caller's, and this is that
                            // caller: press anywhere off the card and the menu
                            // goes away.
                            .on_mouse_down_out(
                                cx.listener(|view, _, _, cx| view.close_context_menu(cx)),
                            )
                            .into_any_element(),
                        closing,
                    ))
                },
            )
            .when(self.dialog.get().is_some(), |root| {
                root.child(popover::modal(
                    "gallery-dialog",
                    window.viewport_size(),
                    popover::dialog_card(&theme)
                        .gap(px(12.0))
                        .child(popover::dialog_title(&theme, "Discard changes?"))
                        .child(popover::dialog_body(
                            &theme,
                            "This cannot be undone. The working tree keeps whatever \
                             you have not saved.",
                        ))
                        .child(
                            div()
                                .mt(px(4.0))
                                .flex()
                                .flex_row()
                                .justify_end()
                                .gap(px(8.0))
                                .child(
                                    div()
                                        .id("dialog-cancel")
                                        .on_click(
                                            cx.listener(|view, _, _, cx| view.close_dialog(cx)),
                                        )
                                        .child(theme.button(
                                            "Cancel",
                                            ButtonStyle::Ghost,
                                            Some("g-dialog-no".into()),
                                        )),
                                )
                                .child(
                                    div()
                                        .id("dialog-confirm")
                                        .on_click(
                                            cx.listener(|view, _, _, cx| view.close_dialog(cx)),
                                        )
                                        .child(theme.button(
                                            "Discard",
                                            ButtonStyle::Destructive,
                                            None,
                                        )),
                                ),
                        )
                        .into_any_element(),
                    cx.listener(|view, _, _, cx| view.close_dialog(cx)),
                ))
            })
            .when(self.sheet.get().is_some(), |root| {
                root.child(popover::sheet(
                    "gallery-sheet",
                    window.viewport_size(),
                    popover::Side::Right,
                    px(320.0),
                    popover::sheet_panel(&theme, popover::Side::Right)
                        .p(px(20.0))
                        .gap(px(14.0))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_between()
                                .child(popover::dialog_title(&theme, "Details"))
                                .child(
                                    div()
                                        .id("close-sheet")
                                        .on_click(
                                            cx.listener(|view, _, _, cx| view.close_sheet(cx)),
                                        )
                                        .child(theme.button(
                                            "Close",
                                            ButtonStyle::Ghost,
                                            Some("g-sheet-close".into()),
                                        )),
                                ),
                        )
                        .child(popover::dialog_body(
                            &theme,
                            "A sheet is the dialog card pinned to an edge — same scrim, \
                             same glass, full height.",
                        ))
                        .child(
                            theme
                                .group_box()
                                .child(
                                    theme
                                        .card_row(true)
                                        .hover(widgets::card_row_hover)
                                        .child(theme.row_tile(icons::MONITOR))
                                        .child(theme.row_title("Appearance")),
                                )
                                .child(
                                    theme
                                        .card_row(false)
                                        .hover(widgets::card_row_hover)
                                        .child(theme.row_tile(icons::FOLDER))
                                        .child(theme.row_title("Storage")),
                                ),
                        )
                        .into_any_element(),
                    self.sheet.closing_since(),
                    cx.listener(|view, _, _, cx| view.close_sheet(cx)),
                ))
            })
            .when_some(self.palette.clone(), |root, palette| {
                // Centered over a scrim, the way a palette always appears.
                root.child(
                    div()
                        .absolute()
                        .inset_0()
                        .bg(theme::scrim(0.35))
                        .flex()
                        .justify_center()
                        // Without items_start the card stretches to the full
                        // window height (flex default is align: stretch).
                        .items_start()
                        .pt(px(120.0))
                        // The palette binds `escape` itself, but a scrim you
                        // can press and nothing happens reads as a stuck
                        // window. The wrapper sizes to the card, so "out" is
                        // the scrim.
                        .child(div().child(palette).on_mouse_down_out(cx.listener(
                            |view, _, _, cx| {
                                view.palette = None;
                                cx.notify();
                            },
                        ))),
                )
            })
    }
}
