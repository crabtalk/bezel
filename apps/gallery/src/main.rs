//! The bezel gallery — every component rendered in a real window. This is the
//! dev surface: new components land here the day they land in `crates/ui`.

use bezel_theme::{Theme, appearance};
use bezel_ui::combobox::Combobox;
use bezel_ui::hover_card::HoverCard;
use bezel_ui::input::TextField;
use bezel_ui::palette::{CommandPalette, PaletteEvent};
use bezel_ui::tooltip::Tooltip;
use bezel_ui::widgets::SplitDrag;
use bezel_ui::{combobox, icons, input, loaders, palette, popover, widgets};
use gpui::{
    App, Axis, Bounds, Context, DragMoveEvent, Empty, Entity, Focusable, KeyBinding, SharedString,
    Window, WindowBounds, WindowOptions, actions, div, prelude::*, px, relative, size,
};

actions!(gallery, [OpenPalette]);

const LANGUAGES: [&str; 8] = [
    "Rust",
    "TypeScript",
    "Swift",
    "Zig",
    "Go",
    "Python",
    "Haskell",
    "OCaml",
];

const COMMANDS: [&str; 8] = [
    "Open File…",
    "Open Recent",
    "Save All",
    "Toggle Sidebar",
    "Toggle Theme",
    "Reload Window",
    "Copy Path",
    "Quit",
];

fn main() {
    gpui_platform::application()
        .with_assets(icons::Assets)
        .run(|cx: &mut App| {
            if let Err(err) = bezel_ui::register_fonts(cx) {
                eprintln!("FONT REGISTRATION FAILED: {err:?}");
            }
            appearance::init(appearance::AppearanceMode::System, cx);
            input::init(cx);
            palette::init(cx);
            combobox::init(cx);
            cx.bind_keys([KeyBinding::new("cmd-k", OpenPalette, None)]);
            let bounds = Bounds::centered(None, size(px(1000.0), px(860.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    // Glass needs a blurred window background to blur INTO;
                    // without it `material` has nothing behind it.
                    window_background: Theme::of(cx).window_background_appearance(),
                    ..Default::default()
                },
                |window, cx| {
                    appearance::observe_window(window, cx).detach();
                    let gallery = cx.new(|cx| Gallery {
                        search: cx
                            .new(|cx| TextField::new(cx).with_placeholder("Search components…")),
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
                    });
                    // Focus a field on launch so the caret is visible.
                    let focus = gallery.read(cx).search.focus_handle(cx);
                    window.focus(&focus, cx);
                    gallery
                },
            )
            .unwrap();
            cx.activate(true);
        });
}

struct Gallery {
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
}

const THEME_CHOICES: [&str; 3] = ["System", "Light", "Dark"];

impl Gallery {
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
}

fn section(theme: &Theme, title: &str) -> gpui::Div {
    div().flex().flex_col().gap(px(12.0)).child(
        div()
            .text_size(px(11.0))
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(theme.text_faint)
            .child(SharedString::from(popover::tracked_upper(title))),
    )
}

fn row() -> gpui::Div {
    div().flex().flex_row().items_center().gap(px(12.0))
}

impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let view = cx.entity_id();

        let buttons = section(&theme, "Buttons").child(
            row()
                .child(popover::button(&theme, "Ghost", "g-ghost"))
                .child(popover::button_prominent(&theme, "Prominent"))
                .child(popover::button_destructive(&theme, "Destructive")),
        );

        let toggles = section(&theme, "Toggle & badges").child(
            row()
                .child(widgets::toggle(&theme, true))
                .child(widgets::toggle(&theme, false))
                .child(widgets::badge(&theme, "badge"))
                .child(widgets::badge_active(&theme, "active")),
        );

        let fields = section(&theme, "Text field").child(
            div()
                .w(px(320.0))
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(self.search.clone())
                .child(self.filled.clone()),
        );

        let menu_open = self.theme_menu.is_open() || self.theme_menu.is_closing();
        let select = section(&theme, "Select").child(
            div().w(px(200.0)).relative().child(
                div()
                    .id("theme-select")
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|view, _, _, _| view.theme_menu.note_trigger_press()),
                    )
                    .on_click(cx.listener(|view, _, _, cx| view.toggle_theme_menu(cx)))
                    .child(widgets::select_trigger(
                        &theme,
                        THEME_CHOICES[self.theme_choice],
                        menu_open,
                    ))
                    .when(menu_open, |trigger| {
                        trigger.child(popover::anchored_menu_below(
                            "theme-select-menu",
                            popover::popover_card(&theme)
                                .w(px(200.0))
                                // Dismissal is the caller's, and the caller is
                                // this view — without it, clicking away leaves
                                // the menu hanging open.
                                .on_mouse_down_out(cx.listener(|view, _, _, cx| {
                                    if view.theme_menu.begin_close() {
                                        popover::reap_popup(cx, |view: &mut Self| {
                                            &mut view.theme_menu
                                        });
                                    }
                                    cx.notify();
                                }))
                                .children(THEME_CHOICES.iter().enumerate().map(|(index, label)| {
                                    popover::menu_row(
                                        &theme,
                                        index == self.theme_choice,
                                        SharedString::from(format!("theme-row-{index}")),
                                    )
                                    .id(SharedString::from(format!("theme-{index}")))
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        view.choose_theme(index, cx)
                                    }))
                                    .child(*label)
                                    .into_any_element()
                                }))
                                .into_any_element(),
                            self.theme_menu.closing_since(),
                        ))
                    }),
            ),
        );

        let combobox = section(&theme, "Combobox")
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
            );

        let controls = section(&theme, "Checkbox, radio, avatar").child(
            row()
                .child(widgets::checkbox(&theme, true))
                .child(widgets::checkbox(&theme, false))
                .child(widgets::radio_button(&theme, true))
                .child(widgets::radio_button(&theme, false))
                .child(widgets::avatar(&theme, "TC"))
                .child(widgets::avatar(&theme, "K")),
        );

        let tracks = section(&theme, "Progress & slider").child(
            div()
                .w(px(280.0))
                .flex()
                .flex_col()
                .gap(px(16.0))
                .child(widgets::progress_bar(&theme, 0.35))
                .child(widgets::progress_bar(&theme, 0.8))
                .child(widgets::slider(&theme, 0.5)),
        );

        let palette_hint = section(&theme, "Command palette").child(
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
        );

        let segments = section(&theme, "Toggle group").child(
            widgets::toggle_group(&theme)
                .child(
                    div()
                        .id("seg-0")
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.segment = 0;
                            cx.notify();
                        }))
                        .child(widgets::toggle_group_item(&theme, "Day", self.segment == 0)),
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
        );

        let collapsible = section(&theme, "Collapsible & breadcrumb")
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
            .child(
                widgets::breadcrumb()
                    .child(widgets::breadcrumb_item(&theme, "crates", false))
                    .child(widgets::breadcrumb_separator(&theme))
                    .child(widgets::breadcrumb_item(&theme, "ui", false))
                    .child(widgets::breadcrumb_separator(&theme))
                    .child(widgets::breadcrumb_item(&theme, "widgets.rs", true)),
            );

        let bits = section(&theme, "Tags, status, tooltip").child(
            row()
                .child(widgets::tag(&theme, "rust"))
                .child(widgets::tag(&theme, "gpui"))
                .child(widgets::status_dot(theme.success))
                .child(widgets::status_dot(theme.warning))
                .child(widgets::status_dot(theme.danger))
                .child(
                    div()
                        .id("tip")
                        .tooltip(|window, cx| {
                            Tooltip::with_keystroke("Copy path", "⌘C", window, cx)
                        })
                        .child(popover::button(&theme, "Hover me", "g-tip")),
                )
                .child(
                    div()
                        .id("hover-card")
                        // Hoverable: the pointer can travel into this one.
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
        );

        let empty = section(&theme, "Empty state").child(widgets::group_box(&theme).child(
            widgets::empty_state(
                &theme,
                icons::FOLDER,
                "No repositories",
                "Open a folder to get started.",
            ),
        ));

        let tabs = section(&theme, "Tabs").child(
            widgets::tab_bar(&theme)
                .child(widgets::tab(&theme, "Components", true))
                .child(widgets::tab(&theme, "Tokens", false))
                .child(widgets::tab(&theme, "Motion", false)),
        );

        let menu = section(&theme, "Menu").child(
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
        );

        let group = section(&theme, "Group box").child(
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
        );

        let spinners = section(&theme, "Loaders").child(
            row()
                .child(loaders::pulse_loader("g-pulse", &theme, 8.0, view, cx))
                .child(loaders::gradient_spinner("g-spin", &theme, 5.0, view, cx))
                .child(loaders::mini_gradient_spinner("g-mini", 2.5, view, cx))
                .child(loaders::loading_word(&theme)),
        );

        // Exercises the fork's backdrop-blur primitive: the card blurs the
        // striped band painted behind it.
        let material = section(&theme, "Material").child(
            div()
                .relative()
                .w(px(420.0))
                .h(px(150.0))
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .flex_row()
                        .children((0..14).map(|i| {
                            div().w(px(30.0)).h_full().bg(if i % 2 == 0 {
                                theme.accent
                            } else {
                                theme.warning
                            })
                        })),
                )
                .child(
                    div().absolute().top(px(28.0)).left(px(60.0)).child(
                        bezel_ui::material::material(
                            12.0,
                            bezel_ui::material::MENU_BLUR,
                            popover::popover_card(&theme)
                                .w(px(220.0))
                                .child(popover::menu_row(&theme, false, "mat-a").child("Blurred")),
                        ),
                    ),
                ),
        );

        let pane = |label: SharedString| {
            div()
                .h_full()
                .p(px(12.0))
                .text_size(px(12.5))
                .text_color(theme.text_muted)
                .child(label)
        };
        let split = section(&theme, "Resizable split").child(
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
                .on_drag_move(
                    cx.listener(|view, event: &DragMoveEvent<SplitDrag>, _, cx| {
                        view.split = widgets::split_fraction(
                            event.event.position,
                            event.bounds,
                            Axis::Horizontal,
                            0.15,
                        );
                        view.split_dragging = true;
                        cx.notify();
                    }),
                )
                // Both, because the release can land anywhere: a divider left
                // lit after the drag ends reads as still grabbed.
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
                .child(
                    div()
                        .w(relative(self.split))
                        .child(pane(SharedString::from(format!(
                            "{:.0}%",
                            self.split * 100.0
                        )))),
                )
                .child(
                    widgets::split_handle(&theme, Axis::Horizontal, self.split_dragging)
                        .id("split-handle")
                        .on_drag(SplitDrag, |_, _, _, cx| cx.new(|_| Empty)),
                )
                .child(div().flex_1().child(pane("drag the divider".into()))),
        );

        let sheet = section(&theme, "Sheet").child(
            row().child(
                div()
                    .id("open-sheet")
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.sheet.open(());
                        cx.notify();
                    }))
                    .child(popover::button(&theme, "Open sheet", "g-sheet")),
            ),
        );

        let strips = section(&theme, "Strips & redacted")
            .child(widgets::error_strip(&theme, "Something went wrong."))
            .child(widgets::warning_strip(&theme, "Heads up, check this."))
            .child(popover::redacted_rows("g-redacted", &theme, 3, view, cx));

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
            .overflow_y_scroll()
            .bg(theme.bg)
            .font_family(theme.font_sans.clone())
            .text_color(theme.text)
            .text_size(px(14.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(28.0))
                    .p(px(32.0))
                    .child(
                        div()
                            .text_size(px(18.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("bezel gallery"),
                    )
                    // Two columns: the set has outgrown a single scroll, and
                    // every component should be visible in one screenful.
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_start()
                            .gap(px(40.0))
                            .child(
                                div()
                                    .w(px(420.0))
                                    .flex()
                                    .flex_col()
                                    .gap(px(28.0))
                                    .child(buttons)
                                    .child(fields)
                                    .child(toggles)
                                    .child(select)
                                    .child(combobox)
                                    .child(controls)
                                    .child(tracks)
                                    .child(segments)
                                    .child(collapsible)
                                    .child(bits),
                            )
                            .child(
                                div()
                                    .w(px(420.0))
                                    .flex()
                                    .flex_col()
                                    .gap(px(28.0))
                                    .child(tabs)
                                    .child(split)
                                    .child(menu)
                                    .child(group)
                                    .child(empty)
                                    .child(spinners)
                                    .child(palette_hint)
                                    .child(sheet)
                                    .child(material)
                                    .child(strips),
                            ),
                    ),
            )
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
