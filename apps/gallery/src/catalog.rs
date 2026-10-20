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
    /// component demo is designed for. Such a page is a screen: it fills the
    /// pane, scrolls its own parts, and floats its own chrome over them.
    pub full_bleed: bool,
}

/// The top nav: tokens, components, then a tab per domain whose pages are
/// screens.
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
        title: "Editor",
        groups: EDITOR,
        home: "editor",
        full_bleed: true,
    },
    Tab {
        title: "Diagrams",
        groups: DIAGRAMS,
        home: "canvas",
        full_bleed: true,
    },
    Tab {
        title: "Agent",
        groups: AGENT,
        home: "agent-avatar",
        full_bleed: true,
    },
];

/// Agent screens. A page appears here when the parts under it are real —
/// nothing is a row until it can be pressed.
pub const AGENT: &[Group] = &[
    Group {
        title: "Conversation",
        sections: &[
            section(
                "agent-activity",
                "Activity",
                "apps/gallery/src/pages/agent/activity.rs",
            ),
            section(
                "agent-tools",
                "Tool calls",
                "apps/gallery/src/pages/agent/activity.rs",
            ),
            section(
                "agent-composer",
                "Composer",
                "apps/gallery/src/pages/agent/activity.rs",
            ),
            section(
                "agent-transcript",
                "Transcript",
                "apps/gallery/src/pages/agent/transcript.rs",
            ),
        ],
    },
    Group {
        title: "Tools",
        sections: &[
            section("agent-diff", "Diff", "apps/gallery/src/pages/agent/diff.rs"),
            // Native-only: the terminal crate sits off the wasm build
            // (alacritty_terminal pulls `home`, which does not compile for
            // wasm32) — see the gallery manifest.
            #[cfg(not(target_family = "wasm"))]
            section(
                "agent-terminal",
                "Terminal",
                "apps/gallery/src/pages/agent/terminal.rs",
            ),
            section(
                "browser",
                "Browser",
                "apps/gallery/src/pages/agent/browser.rs",
            ),
        ],
    },
    Group {
        title: "Presence",
        sections: &[
            section(
                "agent-orbs",
                "Thinking orbs",
                "apps/gallery/src/pages/agent/orbs.rs",
            ),
            section(
                "agent-avatar",
                "Blob avatars",
                "apps/gallery/src/pages/agent/avatar.rs",
            ),
        ],
    },
];

/// Document screens: the markdown model, read, selected and edited.
pub const EDITOR: &[Group] = &[
    Group {
        title: "Documents",
        sections: &[
            section(
                "editor",
                "Editor",
                "apps/gallery/src/pages/editor/editing.rs",
            ),
            section(
                "document",
                "Document",
                "apps/gallery/src/pages/editor/document.rs",
            ),
            section(
                "selectable-text",
                "Selectable text",
                "apps/gallery/src/pages/editor/selectable.rs",
            ),
            section(
                "ribbon",
                "Ribbon",
                "apps/gallery/src/pages/editor/ribbon.rs",
            ),
        ],
    },
    Group {
        title: "Text",
        sections: &[
            section(
                "markdown",
                "Markdown",
                "apps/gallery/src/pages/editor/dialect.rs",
            ),
            section(
                "syntax",
                "Syntax",
                "apps/gallery/src/pages/editor/syntax.rs",
            ),
        ],
    },
];

/// A canvas, charts and a Mermaid playground drawn live, then one page per
/// diagram type `bezel-mermaid` draws, each pointing at the module that parses
/// and draws it. A `mermaid-` row needs a sample in
/// [`crate::pages::diagrams::mermaid::SAMPLES`].
pub const DIAGRAMS: &[Group] = &[
    Group {
        title: "Live",
        sections: &[
            section(
                "canvas",
                "Canvas",
                "apps/gallery/src/pages/diagrams/mindmap.rs",
            ),
            section(
                "charts",
                "Charts",
                "apps/gallery/src/pages/diagrams/charts.rs",
            ),
            section(
                "playground",
                "Playground",
                "apps/gallery/src/pages/diagrams/playground.rs",
            ),
        ],
    },
    Group {
        title: "Mermaid",
        sections: &[section(
            "mermaid",
            "Overview",
            "crates/markdown/src/fence/mermaid/mod.rs",
        )],
    },
    Group {
        title: "Graphs",
        sections: &[
            section(
                "mermaid-flowchart",
                "Flowchart",
                "crates/mermaid/src/diagrams/flowchart/mod.rs",
            ),
            section(
                "mermaid-sequence",
                "Sequence",
                "crates/mermaid/src/diagrams/sequence/mod.rs",
            ),
            section(
                "mermaid-class",
                "Class",
                "crates/mermaid/src/diagrams/class/mod.rs",
            ),
            section(
                "mermaid-state",
                "State",
                "crates/mermaid/src/diagrams/state/mod.rs",
            ),
            section(
                "mermaid-er",
                "Entity relationship",
                "crates/mermaid/src/diagrams/er/mod.rs",
            ),
            section(
                "mermaid-requirement",
                "Requirement",
                "crates/mermaid/src/diagrams/requirement/mod.rs",
            ),
            section(
                "mermaid-architecture",
                "Architecture",
                "crates/mermaid/src/diagrams/architecture/mod.rs",
            ),
            section(
                "mermaid-block",
                "Block",
                "crates/mermaid/src/diagrams/block/mod.rs",
            ),
            section("mermaid-c4", "C4", "crates/mermaid/src/diagrams/c4/mod.rs"),
        ],
    },
    Group {
        title: "Process",
        sections: &[
            section(
                "mermaid-usecase",
                "Use case",
                "crates/mermaid/src/diagrams/usecase/mod.rs",
            ),
            section(
                "mermaid-swimlane",
                "Swimlane",
                "crates/mermaid/src/diagrams/swimlane/mod.rs",
            ),
            section(
                "mermaid-agentflow",
                "Agent flow",
                "crates/mermaid/src/diagrams/agentflow/mod.rs",
            ),
            section(
                "mermaid-eventmodeling",
                "Event modeling",
                "crates/mermaid/src/diagrams/eventmodeling/mod.rs",
            ),
            section(
                "mermaid-journey",
                "User journey",
                "crates/mermaid/src/diagrams/journey/mod.rs",
            ),
            section(
                "mermaid-gitgraph",
                "Git graph",
                "crates/mermaid/src/diagrams/git/mod.rs",
            ),
        ],
    },
    Group {
        title: "Charts",
        sections: &[
            section(
                "mermaid-pie",
                "Pie",
                "crates/mermaid/src/diagrams/pie/mod.rs",
            ),
            section(
                "mermaid-xychart",
                "XY chart",
                "crates/mermaid/src/diagrams/xychart/mod.rs",
            ),
            section(
                "mermaid-quadrant",
                "Quadrant",
                "crates/mermaid/src/diagrams/quadrant/mod.rs",
            ),
            section(
                "mermaid-radar",
                "Radar",
                "crates/mermaid/src/diagrams/radar/mod.rs",
            ),
            section(
                "mermaid-sankey",
                "Sankey",
                "crates/mermaid/src/diagrams/sankey/mod.rs",
            ),
            section(
                "mermaid-treemap",
                "Treemap",
                "crates/mermaid/src/diagrams/treemap/mod.rs",
            ),
            section(
                "mermaid-venn",
                "Venn",
                "crates/mermaid/src/diagrams/venn/mod.rs",
            ),
            section(
                "mermaid-packet",
                "Packet",
                "crates/mermaid/src/diagrams/packet/mod.rs",
            ),
        ],
    },
    Group {
        title: "Planning",
        sections: &[
            section(
                "mermaid-gantt",
                "Gantt",
                "crates/mermaid/src/diagrams/gantt/mod.rs",
            ),
            section(
                "mermaid-timeline",
                "Timeline",
                "crates/mermaid/src/diagrams/timeline/mod.rs",
            ),
            section(
                "mermaid-kanban",
                "Kanban",
                "crates/mermaid/src/diagrams/kanban/mod.rs",
            ),
        ],
    },
    Group {
        title: "Maps & trees",
        sections: &[
            section(
                "mermaid-mindmap",
                "Mindmap",
                "crates/mermaid/src/diagrams/mindmap/mod.rs",
            ),
            section(
                "mermaid-treeview",
                "Tree view",
                "crates/mermaid/src/diagrams/treeview/mod.rs",
            ),
            section(
                "mermaid-ishikawa",
                "Ishikawa",
                "crates/mermaid/src/diagrams/ishikawa/mod.rs",
            ),
            section(
                "mermaid-wardley",
                "Wardley map",
                "crates/mermaid/src/diagrams/wardley/mod.rs",
            ),
            section(
                "mermaid-cynefin",
                "Cynefin",
                "crates/mermaid/src/diagrams/cynefin/mod.rs",
            ),
            section(
                "mermaid-railroad",
                "Railroad",
                "crates/mermaid/src/diagrams/railroad/mod.rs",
            ),
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
                "multi-select",
                "Multi-select",
                "crates/ui/src/multi_select.rs",
            ),
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
            section("color-picker", "Color picker", "crates/ui/src/color.rs"),
            section("date-picker", "Date picker", "crates/ui/src/date.rs"),
        ],
    },
    Group {
        title: "Menus & actions",
        sections: &[
            section("menu", "Menu", "crates/ui/src/popover.rs"),
            section(
                "context-menu",
                "Context menu",
                "crates/ui/src/context_menu.rs",
            ),
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
            section("docking", "Pane docking", "crates/ui/src/docking.rs"),
            section("sortable", "Sortable lists", "crates/ui/src/drag.rs"),
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
