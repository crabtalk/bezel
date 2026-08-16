//! The gallery — bezel's documentation, and its dev surface: a rail of every
//! component on the left, the selected one in the pane.
//!
//! [`TABS`] is the catalog: a top nav for the *kind* of thing, a rail for the
//! items in it. A component lands here the day it lands in `crates/ui`,
//! composed exactly once, so the browser is never out of date with the library
//! it documents.

use bezel_theme::Theme;
use bezel_theme::appearance::{self, AppearanceMode};
use bezel_ui::combobox::Combobox;
use bezel_ui::hover_card::HoverCard;
use bezel_ui::input::TextField;
use bezel_ui::palette::{CommandPalette, PaletteEvent};
use bezel_ui::tooltip::Tooltip;
use bezel_ui::widgets::SplitDrag;
use bezel_ui::{icons, loaders, popover, widgets};
use gpui::{
    AnyElement, App, Axis, Context, DragMoveEvent, Empty, Entity, Focusable, SharedString, Window,
    actions, div, prelude::*, px, relative,
};

actions!(gallery, [OpenPalette]);

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

/// One page of the browser.
pub struct Section {
    /// Rail key, and what [`Gallery::section_body`] matches on.
    pub key: &'static str,
    pub title: &'static str,
    /// Where the component is written. Customisation here is editing the
    /// source, so the path is the most useful line of documentation there is.
    pub source: &'static str,
}

/// A rail group.
pub struct Group {
    pub title: &'static str,
    pub sections: &'static [Section],
}

const fn section(key: &'static str, title: &'static str, source: &'static str) -> Section {
    Section { key, title, source }
}

/// A top-nav tab, holding its own rail.
pub struct Tab {
    pub title: &'static str,
    pub groups: &'static [Group],
    /// The page the tab opens on.
    pub home: &'static str,
}

/// The top nav. Two tabs, not shadcn's eight: the axis separates the *kind* of
/// thing you are looking at, and bezel has two kinds until composed patterns
/// exist to fill a third.
pub const TABS: &[Tab] = &[
    Tab {
        title: "Foundations",
        groups: FOUNDATIONS,
        home: "color",
    },
    Tab {
        title: "Components",
        groups: COMPONENTS,
        home: "buttons",
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
            section("buttons", "Buttons", "crates/ui/src/popover.rs"),
            section("text-field", "Text field", "crates/ui/src/input.rs"),
            section("select", "Select", "crates/ui/src/widgets.rs"),
            section("combobox", "Combobox", "crates/ui/src/combobox.rs"),
            section(
                "checkbox-radio",
                "Checkbox & radio",
                "crates/ui/src/widgets.rs",
            ),
            section("toggle", "Toggle", "crates/ui/src/widgets.rs"),
            section("toggle-group", "Toggle group", "crates/ui/src/widgets.rs"),
            section("slider", "Slider", "crates/ui/src/widgets.rs"),
        ],
    },
    Group {
        title: "Menus & actions",
        sections: &[
            section("menu", "Menu", "crates/ui/src/popover.rs"),
            section("context-menu", "Context menu", "crates/ui/src/popover.rs"),
            section("palette", "Command palette", "crates/ui/src/palette.rs"),
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
            section("group-box", "Group box", "crates/ui/src/widgets.rs"),
            section("tabs", "Tabs", "crates/ui/src/widgets.rs"),
            section("collapsible", "Collapsible", "crates/ui/src/widgets.rs"),
            section("split", "Resizable split", "crates/ui/src/widgets.rs"),
        ],
    },
    Group {
        title: "Content",
        sections: &[
            section("badge", "Badge", "crates/ui/src/widgets.rs"),
            section("tag", "Tag", "crates/ui/src/widgets.rs"),
            section("avatar", "Avatar", "crates/ui/src/widgets.rs"),
            section("breadcrumb", "Breadcrumb", "crates/ui/src/widgets.rs"),
            section("empty-state", "Empty state", "crates/ui/src/widgets.rs"),
            section("skeleton", "Skeleton", "crates/ui/src/popover.rs"),
        ],
    },
    Group {
        title: "Status",
        sections: &[
            section("progress", "Progress", "crates/ui/src/widgets.rs"),
            section("status-dot", "Status dot", "crates/ui/src/widgets.rs"),
            section("alerts", "Alert strips", "crates/ui/src/widgets.rs"),
            section("loaders", "Loaders", "crates/ui/src/loaders.rs"),
        ],
    },
];

fn section_at(key: &str) -> Option<&'static Section> {
    TABS.iter()
        .flat_map(|tab| tab.groups)
        .flat_map(|group| group.sections)
        .find(|section| section.key == key)
}

pub struct Gallery {
    search: Entity<TextField>,
    filled: Entity<TextField>,
    /// Mounted only while open — a palette that lingers keeps a stale query.
    palette: Option<Entity<CommandPalette>>,
    last_command: Option<SharedString>,
    segment: usize,
    expanded: bool,
    /// Right-click menu, anchored at the click position.
    context_menu: popover::Popup<gpui::Point<gpui::Pixels>>,
    /// Select state lives here, not in a component: the menu is mounted by
    /// this view, so this view owns whether it is open and what is chosen.
    theme_menu: popover::Popup<()>,
    theme_choice: usize,
    /// The combobox, by contrast, owns its own menu — it has a query field to
    /// hold, so it is an entity.
    language: Entity<Combobox>,
    sheet: popover::Popup<()>,
    /// Where the split's divider sits, as a fraction of the container.
    split: f32,
    split_dragging: bool,
    /// Which top-nav tab is open.
    tab: usize,
    /// Where you were in each tab — switching away and back should land you
    /// where you left, not at the top.
    selected: Vec<&'static str>,
    dialog: popover::Popup<()>,
}

impl Gallery {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            search: cx.new(|cx| TextField::new(cx).with_placeholder("Search components…")),
            filled: cx.new(|cx| {
                let mut field = TextField::new(cx);
                field.set_content("Select me with shift-left", cx);
                field
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
            palette: None,
            last_command: None,
            segment: 0,
            expanded: true,
            context_menu: popover::Popup::default(),
            sheet: popover::Popup::default(),
            split: 0.4,
            split_dragging: false,
            tab: 1,
            selected: TABS.iter().map(|tab| tab.home).collect(),
            dialog: popover::Popup::default(),
        }
    }

    /// The search field's handle — focus it on launch so a caret is visible.
    pub fn search_focus_handle(&self, cx: &App) -> gpui::FocusHandle {
        self.search.focus_handle(cx)
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
            .id("gallery-rail")
            .flex_none()
            .w(px(220.0))
            .h_full()
            .overflow_y_scroll()
            .border_r_1()
            .border_color(theme.border)
            .child(div().flex().flex_col().gap(px(2.0)).p(px(10.0)).children(
                tab.groups.iter().flat_map(|group| {
                    let heading = popover::menu_heading(theme, group.title).into_any_element();
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
                        .child(SharedString::from(section.title))
                        .into_any_element()
                    });
                    std::iter::once(heading).chain(rows)
                }),
            ))
            .into_any_element()
    }

    /// The top nav: the wordmark, the kind of thing you are browsing, and the
    /// appearance switch. Everything here is global — per-page detail belongs
    /// in [`Self::header`].
    fn nav(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let current = self.tab;
        let mode = appearance::mode(cx);
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
            .child(
                widgets::toggle_group(theme).children(AppearanceMode::ALL.into_iter().map(
                    |option| {
                        div()
                            .id(SharedString::from(format!("appearance-{}", option.label())))
                            .on_click(cx.listener(move |_, _, _, cx| {
                                appearance::set_mode(option, cx);
                                cx.notify();
                            }))
                            .child(widgets::toggle_group_item(
                                theme,
                                option.label(),
                                option == mode,
                            ))
                            .into_any_element()
                    },
                )),
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
                    // Customisation is editing the source, so say which file.
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_size(px(11.5))
                            .font_family(theme.font_mono.clone())
                            .text_color(theme.text_faint)
                            .child(SharedString::from(section.source)),
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
                            ("EASE", bezel_motion::EASE),
                            ("EASE_OUT", bezel_motion::EASE_OUT),
                            ("EASE_OUT_EXPO", bezel_motion::EASE_OUT_EXPO),
                            ("EASE_IN_OUT", bezel_motion::EASE_IN_OUT),
                            ("EASE_RESORT", bezel_motion::EASE_RESORT),
                            ("EASE_TAILWIND", bezel_motion::EASE_TAILWIND),
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
            "buttons" => section
                .child(
                    row()
                        .child(popover::button(&theme, "Ghost", "g-ghost"))
                        .child(popover::button_prominent(&theme, "Prominent"))
                        .child(popover::button_destructive(&theme, "Destructive")),
                )
                .into_any_element(),

            "text-field" => section
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
                .child(
                    row()
                        .child(widgets::toggle(&theme, true))
                        .child(widgets::toggle(&theme, false)),
                )
                .into_any_element(),

            "badge" => section
                .child(
                    row()
                        .child(widgets::badge(&theme, "badge"))
                        .child(widgets::badge_active(&theme, "active")),
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
                                .child(widgets::select_trigger(
                                    &theme,
                                    SELECT_CHOICES[self.theme_choice],
                                    menu_open,
                                ))
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
                .child(
                    row()
                        .child(widgets::checkbox(&theme, true))
                        .child(widgets::checkbox(&theme, false))
                        .child(widgets::radio_button(&theme, true))
                        .child(widgets::radio_button(&theme, false)),
                )
                .into_any_element(),

            "avatar" => section
                .child(
                    row()
                        .child(widgets::avatar(&theme, "TC"))
                        .child(widgets::avatar(&theme, "K")),
                )
                .into_any_element(),

            "progress" => section
                .child(
                    div()
                        .w(px(280.0))
                        .flex()
                        .flex_col()
                        .gap(px(16.0))
                        .child(widgets::progress_bar(&theme, 0.35))
                        .child(widgets::progress_bar(&theme, 0.8)),
                )
                .into_any_element(),

            "slider" => section
                .child(div().w(px(280.0)).child(widgets::slider(&theme, 0.5)))
                .into_any_element(),

            "toggle-group" => section
                .child(
                    widgets::toggle_group(&theme)
                        .child(
                            div()
                                .id("seg-0")
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.segment = 0;
                                    cx.notify();
                                }))
                                .child(widgets::toggle_group_item(
                                    &theme,
                                    "Day",
                                    self.segment == 0,
                                )),
                        )
                        .child(
                            div()
                                .id("seg-1")
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.segment = 1;
                                    cx.notify();
                                }))
                                .child(widgets::toggle_group_item(
                                    &theme,
                                    "Week",
                                    self.segment == 1,
                                )),
                        )
                        .child(
                            div()
                                .id("seg-2")
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.segment = 2;
                                    cx.notify();
                                }))
                                .child(widgets::toggle_group_item(
                                    &theme,
                                    "Month",
                                    self.segment == 2,
                                )),
                        ),
                )
                .into_any_element(),

            "collapsible" => section
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
                                .child(widgets::collapsible_header(
                                    &theme,
                                    "Advanced",
                                    self.expanded,
                                )),
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
                .into_any_element(),

            "breadcrumb" => section
                .child(
                    widgets::breadcrumb()
                        .child(widgets::breadcrumb_item(&theme, "crates", false))
                        .child(widgets::breadcrumb_separator(&theme))
                        .child(widgets::breadcrumb_item(&theme, "ui", false))
                        .child(widgets::breadcrumb_separator(&theme))
                        .child(widgets::breadcrumb_item(&theme, "widgets.rs", true)),
                )
                .into_any_element(),

            "tag" => section
                .child(
                    row()
                        .child(widgets::tag(&theme, "rust"))
                        .child(widgets::tag(&theme, "gpui")),
                )
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
                            .child(popover::button(&theme, "Hover me", "g-tip")),
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
                            .child(widgets::tag(&theme, "@clearloop")),
                    ),
                )
                .into_any_element(),

            "tabs" => section
                .child(
                    widgets::tab_bar(&theme)
                        .child(widgets::tab(&theme, "Components", true))
                        .child(widgets::tab(&theme, "Tokens", false))
                        .child(widgets::tab(&theme, "Motion", false)),
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
                                    view.split = widgets::split_fraction(
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
                                widgets::split_handle(
                                    &theme,
                                    Axis::Horizontal,
                                    self.split_dragging,
                                )
                                .id("split-handle")
                                .on_drag(SplitDrag, |_, _, _, cx| cx.new(|_| Empty)),
                            )
                            .child(div().flex_1().child(pane("drag the divider".into()))),
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
                    widgets::group_box(&theme)
                        .child(
                            widgets::card_row(&theme, true)
                                .child(widgets::row_tile(&theme, icons::MONITOR))
                                .child(widgets::row_title(&theme, "First row")),
                        )
                        .child(
                            widgets::card_row(&theme, false)
                                .child(widgets::row_tile(&theme, icons::FOLDER))
                                .child(widgets::row_title(&theme, "Second row")),
                        ),
                )
                .into_any_element(),

            "empty-state" => section
                .child(widgets::group_box(&theme).child(widgets::empty_state(
                    &theme,
                    icons::FOLDER,
                    "No repositories",
                    "Open a folder to get started.",
                )))
                .into_any_element(),

            "loaders" => section
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
                            .child(popover::button(&theme, "Open sheet", "g-sheet")),
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
                            .child(popover::button(&theme, "Open dialog", "g-dialog")),
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
                                bezel_ui::material::material(
                                    12.0,
                                    bezel_ui::material::MENU_BLUR,
                                    popover::popover_card(&theme).w(px(220.0)).child(
                                        popover::menu_row(&theme, false, "mat-a").child("Blurred"),
                                    ),
                                ),
                            )),
                    )
                    .into_any_element()
            }

            "alerts" => section
                .child(widgets::error_strip(&theme, "Something went wrong."))
                .child(widgets::warning_strip(&theme, "Heads up, check this."))
                .into_any_element(),

            "skeleton" => section
                .child(popover::redacted_rows("g-redacted", &theme, 3, view, cx))
                .into_any_element(),

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

/// Every named spec in `bezel-motion`.
const MOTION_CATALOG: &[(&str, bezel_motion::MotionSpec)] = &[
    ("FADE_IN", bezel_motion::FADE_IN),
    ("FADE_QUICK", bezel_motion::FADE_QUICK),
    ("MENU_IN", bezel_motion::MENU_IN),
    ("MENU_OUT", bezel_motion::MENU_OUT),
    ("DIALOG_IN", bezel_motion::DIALOG_IN),
    ("SPLASH_OUT", bezel_motion::SPLASH_OUT),
    ("RESIZE", bezel_motion::RESIZE),
    ("TAB_SLIDE", bezel_motion::TAB_SLIDE),
    ("COLLAPSE", bezel_motion::COLLAPSE),
    ("CHEVRON", bezel_motion::CHEVRON),
    ("SCROLL_GLIDE", bezel_motion::SCROLL_GLIDE),
    ("HOVER_FADE", bezel_motion::HOVER_FADE),
    ("PULSE", bezel_motion::PULSE),
    ("GRADIENT_SPIN", bezel_motion::GRADIENT_SPIN),
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
                    bezel_theme::contrast_ratio(color, theme.bg)
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
        let content = div()
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
                            .child(
                                div()
                                    .id("gallery-pane")
                                    .flex_1()
                                    .min_h_0()
                                    .overflow_y_scroll()
                                    // The column width components are designed
                                    // for; several are `w_full` and would
                                    // otherwise stretch to the whole pane.
                                    .child(div().p(px(32.0)).child(column().child(body))),
                            ),
                    ),
            );

        div()
            .id("gallery-scroll")
            .key_context("Gallery")
            .on_action(cx.listener(Self::open_palette))
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
                                        .child(popover::button(&theme, "Cancel", "g-dialog-no")),
                                )
                                .child(
                                    div()
                                        .id("dialog-confirm")
                                        .on_click(
                                            cx.listener(|view, _, _, cx| view.close_dialog(cx)),
                                        )
                                        .child(popover::button_destructive(&theme, "Discard")),
                                ),
                        )
                        .into_any_element(),
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
                                        .child(popover::button(&theme, "Close", "g-sheet-close")),
                                ),
                        )
                        .child(popover::dialog_body(
                            &theme,
                            "A sheet is the dialog card pinned to an edge — same scrim, \
                             same glass, full height.",
                        ))
                        .child(
                            widgets::group_box(&theme)
                                .child(
                                    widgets::card_row(&theme, true)
                                        .child(widgets::row_tile(&theme, icons::MONITOR))
                                        .child(widgets::row_title(&theme, "Appearance")),
                                )
                                .child(
                                    widgets::card_row(&theme, false)
                                        .child(widgets::row_tile(&theme, icons::FOLDER))
                                        .child(widgets::row_title(&theme, "Storage")),
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
                        .bg(bezel_theme::scrim(0.35))
                        .flex()
                        .justify_center()
                        // Without items_start the card stretches to the full
                        // window height (flex default is align: stretch).
                        .items_start()
                        .pt(px(120.0))
                        .child(palette),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_sections() -> impl Iterator<Item = &'static Section> {
        TABS.iter()
            .flat_map(|tab| tab.groups)
            .flat_map(|group| group.sections)
    }

    /// Two rows with the same key would open the same page, and the rail would
    /// highlight both.
    #[test]
    fn rail_keys_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for section in all_sections() {
            assert!(
                seen.insert(section.key),
                "duplicate rail key {}",
                section.key
            );
        }
    }

    /// The header prints a source path as documentation. A moved or renamed
    /// file turns that into a lie, silently — so check every one resolves.
    #[test]
    fn every_section_names_a_file_that_exists() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|apps| apps.parent())
            .expect("workspace root");
        for section in all_sections() {
            let path = root.join(section.source);
            assert!(
                path.exists(),
                "{} points at {}, which does not exist",
                section.key,
                path.display()
            );
        }
    }
}
