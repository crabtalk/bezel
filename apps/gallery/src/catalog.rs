//! The catalog: every tab, group and section the gallery shows.

/// One named demo inside a section, and the unit both surfaces are built from:
/// the gallery stacks them under their titles, and a doc page names one on a
/// snippet to put it in the preview beside it.
pub struct Example {
    /// What a snippet's ``` ```rust example=<key> ``` names, and what `?e=`
    /// embeds. Unique within its section, not across the catalog.
    pub key: &'static str,
    pub title: &'static str,
}

pub(crate) const fn example(key: &'static str, title: &'static str) -> Example {
    Example { key, title }
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
    /// The demos on this page, in the order they are painted. Empty while a
    /// section is still one undivided body: the page renders it whole and its
    /// doc page gets a single preview.
    pub examples: &'static [Example],
}

/// A rail group.
pub struct Group {
    pub title: &'static str,
    pub sections: &'static [Section],
}

pub(crate) const fn section(
    key: &'static str,
    title: &'static str,
    source: &'static str,
) -> Section {
    Section {
        key,
        title,
        source: Some(source),
        examples: &[],
    }
}

/// A section split into named examples — what a restructured page is declared
/// with, and what lets a doc page put each preview beside its own snippet.
pub(crate) const fn section_of(
    key: &'static str,
    title: &'static str,
    source: &'static str,
    examples: &'static [Example],
) -> Section {
    Section {
        key,
        title,
        source: Some(source),
        examples,
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
pub(crate) const fn planned(key: &'static str, title: &'static str) -> Section {
    Section {
        key,
        title,
        source: None,
        examples: &[],
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
        home: "agent-avatar",
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
            section(
                "selectable-text",
                "Selectable text",
                "apps/gallery/src/patterns/selectable.rs",
            ),
            section("editor", "Editor", "apps/gallery/src/patterns/editor.rs"),
            section("canvas", "Canvas", "apps/gallery/src/patterns/canvas.rs"),
            section("browser", "Browser", "apps/gallery/src/patterns/browser.rs"),
            section("ribbon", "Ribbon", "apps/gallery/src/patterns/ribbon.rs"),
            section(
                "markdown",
                "Markdown",
                "apps/gallery/src/patterns/dialect.rs",
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
            section("theme", "Theme", "apps/gallery/src/brand.rs"),
            section("color", "Color", "crates/theme/src/lib.rs"),
            section(
                "typography",
                "Typography",
                "crates/theme/src/theme/typography.rs",
            ),
            section("layout", "Layout", "crates/theme/src/lib.rs"),
            section("material", "Materials", "crates/ui/src/surface.rs"),
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
        sections: &[section("icons", "Icons", "crates/icons/src/lib.rs")],
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
            section_of(
                "buttons",
                "Buttons",
                "crates/ui/src/widgets/buttons.rs",
                &[
                    example("basic", "Basic"),
                    example("styles", "Styles"),
                    example("icon", "Icon"),
                    example("group", "Control group"),
                    example("capsule", "Capsule"),
                    example("lensed", "Lensed"),
                    example("ghost", "Ghost frame"),
                ],
            ),
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
            section("tab-strip", "Tab strip", "crates/ui/src/tabs.rs"),
            section("nav-row", "Nav row", "crates/ui/src/widgets/layout.rs"),
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
            section("titlebar", "Titlebar", "crates/ui/src/titlebar.rs"),
            section("control-bar", "Control bar", "crates/ui/src/control_bar.rs"),
            section("floating", "Floating panel", "crates/ui/src/floating.rs"),
        ],
    },
    // Nothing here is built. It is one whole group on purpose: the data
    // surfaces were deferred together, and they are the next round together.
    Group {
        title: "Data",
        sections: &[
            section("scroll-area", "Scroll area", "crates/ui/src/scroll.rs"),
            section("follow", "Follow scroll", "crates/ui/src/scroll.rs"),
            section("drift", "Drag to scroll", "crates/ui/src/scroll.rs"),
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
            section("stats", "Stats", "crates/ui/src/stats.rs"),
        ],
    },
];

/// The keys [`Gallery::section_body`] answers with a TODO page. Listed rather
/// than derived because the arms cannot be enumerated at runtime; a test keeps
/// this in step with the [`planned`] rows.
pub const PLANNED_BODIES: &[&str] = &[];

pub(crate) fn section_at(key: &str) -> Option<&'static Section> {
    TABS.iter()
        .flat_map(|tab| tab.groups)
        .flat_map(|group| group.sections)
        .find(|section| section.key == key)
}

/// Which tab holds a key, so an embed can select a page without knowing the
/// shape of the catalog above it.
pub(crate) fn tab_of(key: &str) -> Option<usize> {
    TABS.iter().position(|tab| {
        tab.groups
            .iter()
            .any(|group| group.sections.iter().any(|section| section.key == key))
    })
}
