//! Demo data the sections show.

use crate::*;

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

pub(crate) const SELECT_CHOICES: [&str; 3] = ["Comfortable", "Compact", "Dense"];

/// What the tab-strip demo can open — label, glyph, whether it carries the
/// unsaved dot, and its trailing badge if it has one. The `+` opens them in
/// this order.
pub(crate) const STRIP_TABS: [(&str, &[u8], bool, &str); 5] = [
    ("Review", icons::glyph::GitCompare, false, "12"),
    ("Terminal", icons::glyph::Terminal, false, ""),
    ("main.rs", icons::glyph::File, true, ""),
    ("theme.rs", icons::glyph::File, false, ""),
    ("README.md", icons::glyph::FileText, false, ""),
];

/// What the strip demo marks an unsaved tab with. The glyph is the app's, not
/// the crate's — `ui` carries only the icon categories it paints itself.
pub(crate) const STRIP_MARK: &[u8] = icons::glyph::CircleSmall;

/// What the Materials probe's rim slider spans, in points. Wide enough to reach
/// the dome the lens used to paint over the whole shape.
pub(crate) const RIM_RANGE: f32 = 32.0;

/// What one press of ← or → moves the slider. The step is never the library's:
/// bezel dispatches [`focus::Decrement`]/[`focus::Increment`] and the page that
/// owns the value decides what they are worth.
pub(crate) const SLIDER_STEP: f32 = 0.05;

/// What the rest of the probe's knobs span. One range has to hold both shipped
/// looks, which is why `clear` sits high in the gain track and `regular` low.
pub(crate) const GAIN_RANGE: f32 = 1.5;
/// Every surface the probe can paint: the frost scale, then the two glasses.
pub(crate) const PROBE_STYLES: [SurfaceStyle; 7] = [
    SurfaceStyle::Material(Material::UltraThin),
    SurfaceStyle::Material(Material::Thin),
    SurfaceStyle::Material(Material::Regular),
    SurfaceStyle::Material(Material::Thick),
    SurfaceStyle::Material(Material::UltraThick),
    SurfaceStyle::Glass(Glass::Regular),
    SurfaceStyle::Glass(Glass::Clear),
];

/// Its chip label, which is also its element id.
pub(crate) fn probe_style_name(style: SurfaceStyle) -> &'static str {
    match style {
        SurfaceStyle::Material(Material::UltraThin) => "ultraThin",
        SurfaceStyle::Material(Material::Thin) => "thin",
        SurfaceStyle::Material(Material::Regular) => "frost",
        SurfaceStyle::Material(Material::Thick) => "thick",
        SurfaceStyle::Material(Material::UltraThick) => "ultraThick",
        SurfaceStyle::Glass(Glass::Regular) => "regular",
        SurfaceStyle::Glass(Glass::Clear) => "clear",
    }
}

/// Saturation spans grey through the pass-through `clear` measures to well
/// past any boost `regular` could plausibly want.
pub(crate) const SAT_RANGE: f32 = 3.0;
pub(crate) const LIFT_RANGE: f32 = 1.0;
pub(crate) const EDGE_RANGE: f32 = 0.6;
pub(crate) const EDGE_W_RANGE: f32 = 4.0;
pub(crate) const DISPERSION_RANGE: f32 = 0.05;

/// What the probe's blur knob spans, in points. One track for every look, so
/// it has to hold `clear`'s 1.2 and frost's 44 at once.
pub(crate) const BLUR_RANGE: f32 = 60.0;

/// One arrow press on a probe knob: mag by 0.1, width by 1.5pt, rim by 0.1pt —
/// the resolution those are measured at, which is finer than a control needs.
pub(crate) const PROBE_STEP: f32 = 1.0 / 320.0;

/// Pages in the pagination page's imaginary result set — the shape of thing
/// that arrives one page at a time and cannot be held whole.
pub(crate) const RESULT_PAGES: usize = 87;

/// How many rows the virtualized-list page claims to hold. Large enough that
/// building them all would be obvious.
pub(crate) const VIRTUAL_ROWS: usize = 10_000;

/// The tree page's data. A nested structure the *app* owns — bezel never sees
/// one, which is why [`Gallery::tree_rows`] below exists.
pub(crate) struct Node {
    pub(crate) name: &'static str,
    pub(crate) children: &'static [Node],
}

pub(crate) const FILE_TREE: &[Node] = &[
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
pub(crate) struct TreeRow {
    pub(crate) row: tree::Row,
    pub(crate) label: &'static str,
    pub(crate) path: String,
}

/// Flatten the open parts of [`FILE_TREE`] into visible rows.
///
/// This is the function every consumer of `tree` writes, and the reason the
/// module asks for a flat list: bezel cannot walk a tree it knows nothing
/// about, and the app has to produce these rows to render them anyway.
pub(crate) fn flatten_tree(
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
pub(crate) fn table_columns() -> Vec<Column> {
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
pub(crate) const TABLE_ROWS: [(&str, &str, u32); 7] = [
    ("bezel.toml", "Config", 812),
    ("palette.rs", "Source", 24_930),
    ("README.md", "Document", 4_216),
    ("Geist.ttf", "Font", 1_284_400),
    ("icons/", "Folder", 58),
    ("theme.json", "Config", 9_004),
    ("notes.md", "Document", 1_130),
];

/// The drift demo's strip. Enough of them to run well past either end of the
/// pane, because a board that fits on screen has nothing to demonstrate.
pub(crate) const DRIFT_CHIPS: [&str; 16] = [
    "Triage", "Backlog", "Spec", "Design", "Review", "Build", "Blocked", "Staging", "Verify",
    "Docs", "Release", "Watch", "Regress", "Support", "Archive", "Icebox",
];

/// What a chip carries while it is being dragged. A payload type of its own —
/// `on_drag_move` filters by type, and it is what keeps the strip from
/// drifting under a drag that has nothing to do with it.
#[derive(Clone)]
pub struct ChipDrag(pub(crate) SharedString);

/// The chip under the pointer while it is carried. gpui paints this at the
/// window's top layer, which is the reason it is an entity and not a styled
/// copy of the chip: the strip clips its own overflow, and a ghost inside it
/// would be cut off at the edge the drag is heading for.
pub struct HeldChip(pub(crate) SharedString);

impl Render for HeldChip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        div()
            .px(px(12.0))
            .py(px(8.0))
            .rounded(px(Theme::control_radius()))
            .border_1()
            .border_color(theme.accent)
            .bg(theme.surface_raised)
            .text_style(TextStyle::Callout)
            .text_color(theme.text)
            .child(self.0.clone())
    }
}

/// One row on the Step row page. A build rather than an agent turn, on purpose:
/// the component is named for the shape, and the shape is "an operation with an
/// outcome" wherever it turns up.
pub(crate) struct Step {
    pub(crate) icon: &'static [u8],
    pub(crate) title: &'static str,
    pub(crate) detail: &'static str,
    pub(crate) meta: &'static str,
    pub(crate) failed: bool,
    /// `None` is a step that printed nothing, which is what suppresses the
    /// chevron.
    pub(crate) output: Option<&'static str>,
}

pub(crate) const STEPS: [Step; 3] = [
    Step {
        icon: icons::glyph::Terminal,
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
        icon: icons::glyph::Search,
        title: "Search",
        detail: "fn at_bottom",
        meta: "12ms",
        failed: false,
        output: None,
    },
    Step {
        icon: icons::glyph::FileText,
        title: "Read",
        detail: "crates/ui/src/missing.rs",
        meta: "3ms",
        failed: true,
        output: Some("error: no such file or directory (os error 2)"),
    },
];

/// The menubar page's menus. Ordinary app chrome, with the rows worth showing:
/// a separator, a disabled item the keyboard steps straight over, and two
/// submenus — one of them nested a second level down.
///
/// The accelerators are printed, not bound — `menubar` never dispatches, so
/// these name shortcuts this app would wire itself. [`keys::printed`] is what
/// keeps them the platform's own: a row that reaches the keymap instead wants
/// [`Item::with_shortcut`].
pub(crate) fn demo_menus() -> Vec<Menu> {
    vec![
        Menu::new(
            "File",
            vec![
                // The File menu is the one that carries glyphs: the gutter
                // only appears where a row has one, so a demo without it would
                // never show that column.
                Item::action("New Window")
                    .with_icon(icons::glyph::FilePlus)
                    .with_keystroke(keys::printed("secondary-n")),
                // The described row, and the one that shows what a
                // description too long for its line does: it clips, and the
                // tooltip carries the whole of it.
                Item::action("Open…")
                    .with_icon(icons::glyph::FolderOpen)
                    .with_keystroke(keys::printed("secondary-o"))
                    .with_long_description("Choose a markdown file from this workspace to edit"),
                Item::submenu(
                    "Open Recent",
                    vec![
                        Item::action("bezel.md"),
                        Item::action("theme.rs"),
                        Item::Separator,
                        Item::action("Clear Menu"),
                    ],
                ),
                Item::Separator,
                Item::action("Save")
                    .with_icon(icons::glyph::Save)
                    .with_keystroke(keys::printed("secondary-s")),
                Item::action("Save As…")
                    .with_icon(icons::glyph::Save)
                    .with_keystroke(keys::printed("secondary-shift-s"))
                    .disabled(),
            ],
        ),
        Menu::new(
            "Edit",
            vec![
                Item::action("Undo").with_keystroke(keys::printed("secondary-z")),
                Item::action("Redo")
                    .with_keystroke(keys::printed("secondary-shift-z"))
                    .disabled(),
                Item::Separator,
                Item::action("Cut").with_keystroke(keys::printed("secondary-x")),
                Item::action("Copy").with_keystroke(keys::printed("secondary-c")),
                Item::action("Paste").with_keystroke(keys::printed("secondary-v")),
            ],
        ),
        Menu::new(
            "View",
            vec![
                Item::action("Toggle Sidebar").with_keystroke(keys::printed("secondary-b")),
                Item::action("Full Screen").with_keystroke(keys::printed(
                    match cfg!(target_os = "macos") {
                        true => "ctrl-cmd-f",
                        false => "f11",
                    },
                )),
                Item::Separator,
                Item::submenu(
                    "Appearance",
                    vec![
                        Item::action("Light"),
                        Item::action("Dark").checked(true),
                        Item::Separator,
                        Item::submenu(
                            "Accent",
                            vec![Item::action("Blue").checked(true), Item::action("Graphite")],
                        ),
                    ],
                ),
                Item::submenu("Nothing Here", vec![]).disabled(),
            ],
        ),
    ]
}
