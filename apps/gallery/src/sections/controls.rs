use crate::*;

impl Gallery {
    pub(crate) fn controls(
        &mut self,
        key: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let theme = Theme::of(cx).clone();
        let view = Painter::of(cx);
        let section = stack();

        Some(match key {
            // ---- Components --------------------------------------------------
            "text-field" => section
                .child(hint(
                    &theme,
                    "cmd-z undoes a run of typing at a time, not a letter at a time; \
                     moving the caret or switching between typing and deleting ends \
                     the run.",
                ))
                .child(
                    div()
                        .w_full()
                        .max_w(px(320.0))
                        .flex()
                        .flex_col()
                        .gap(px(10.0))
                        .child(self.controls.search.clone())
                        .child(self.controls.filled.clone()),
                )
                .into_any_element(),

            "toggle" => section
                .child(hint(&theme, "space or enter flips the focused switch."))
                .child(row().children((0..2).map(|index| {
                    pressable(
                        focus::focusable(
                            &theme,
                            &self.controls.switches[index],
                            theme.toggle(self.controls.switched[index]),
                        ),
                        SharedString::from(format!("toggle-{index}")),
                        cx,
                        move |view, cx| {
                            view.controls.switched[index] = !view.controls.switched[index];
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
                let menu_open =
                    self.controls.theme_menu.is_open() || self.controls.theme_menu.is_closing();
                section
                    .child(
                        div().w(px(200.0)).relative().child(
                            popover::menu_trigger(
                                div().id("theme-select"),
                                |view: &mut Self| &mut view.controls.theme_menu,
                                |_| (),
                                cx,
                            )
                            .child(theme.select_trigger(SELECT_CHOICES[self.controls.theme_choice]))
                            .when(menu_open, |trigger| {
                                trigger.child(popover::anchored_menu_below(
                                    "theme-select-menu",
                                    // Dismissal is the caller's, and the
                                    // caller is this view — without it,
                                    // clicking away leaves it open.
                                    popover::dismiss_on_out(
                                        popover::popover_card(&theme).w(px(200.0)),
                                        |view: &mut Self| &mut view.controls.theme_menu,
                                        cx,
                                    )
                                    .children(SELECT_CHOICES.iter().enumerate().map(
                                        |(index, label)| {
                                            popover::menu_row(
                                                &theme,
                                                false,
                                                Some(Fade::new(view, format!("theme-row-{index}"))),
                                            )
                                            .justify_between()
                                            .id(SharedString::from(format!("theme-{index}")))
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                view.choose_theme(index, cx)
                                            }))
                                            .child(*label)
                                            .when(index == self.controls.theme_choice, |row| {
                                                row.child(
                                                    icons::icon(icons::glyph::Check)
                                                        .size(px(13.0))
                                                        .text_color(theme.text),
                                                )
                                            })
                                            .into_any_element()
                                        },
                                    ))
                                    .into_any_element(),
                                    self.controls.theme_menu.closing_since(),
                                ))
                            }),
                        ),
                    )
                    .into_any_element()
            }

            "combobox" => section
                .child(div().w(px(220.0)).child(self.controls.language.clone()))
                .child(
                    div()
                        .text_style(TextStyle::Callout)
                        .text_color(theme.text_muted)
                        .child(SharedString::from(
                            match self.controls.language.read(cx).selection() {
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
                                    &self.controls.checkboxes[index],
                                    theme.checkbox(self.controls.checked[index]),
                                ),
                                SharedString::from(format!("checkbox-{index}")),
                                cx,
                                move |view, cx| {
                                    view.controls.checked[index] = !view.controls.checked[index];
                                    cx.notify();
                                },
                            )
                            .into_any_element()
                        }))
                        .children((0..2).map(|index| {
                            pressable(
                                focus::focusable(
                                    &theme,
                                    &self.controls.radios[index],
                                    theme.radio_button(self.controls.radio == index),
                                ),
                                SharedString::from(format!("radio-{index}")),
                                cx,
                                move |view, cx| {
                                    view.controls.radio = index;
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
                        focus::focusable(
                            &theme,
                            &self.controls.slider,
                            theme.slider(self.controls.level),
                        )
                        .id("slider")
                        // The element is its own drag source, so the gesture
                        // starts wherever the pointer went down on the track.
                        .on_drag(SliderDrag("slider".into()), |_, _, _, cx| cx.new(|_| Empty))
                        .on_drag_move(cx.listener(
                            |view, event: &DragMoveEvent<SliderDrag>, _, cx| {
                                let Some(fraction) = widgets::slider_fraction(event, "slider", cx)
                                else {
                                    return;
                                };
                                view.controls.level = fraction;
                                cx.notify();
                            },
                        ))
                        .on_action(cx.listener(|view, _: &focus::Decrement, _, cx| {
                            view.nudge(-SLIDER_STEP, cx)
                        }))
                        .on_action(cx.listener(
                            |view, _: &focus::Increment, _, cx| view.nudge(SLIDER_STEP, cx),
                        )),
                    ),
                )
                .child(
                    div()
                        .text_style(TextStyle::Callout)
                        .font_family(theme.font_mono.clone())
                        .text_color(theme.text_muted)
                        .child(SharedString::from(format!(
                            "{:.0}%",
                            self.controls.level * 100.0
                        ))),
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
                                        &self.controls.segments[index],
                                        theme.toggle_group_item(
                                            label,
                                            self.controls.segment == index,
                                        ),
                                    ),
                                    SharedString::from(format!("segment-{index}")),
                                    cx,
                                    move |view, cx| {
                                        view.controls.segment = index;
                                        cx.notify();
                                    },
                                )
                                .into_any_element()
                            }),
                    ),
                )
                .child(hint(
                    &theme,
                    "A segment can carry a glyph instead of a word, for a control \
                     with no room for one. The tooltip is the caller's — a glyph \
                     nobody recognises says nothing without it.",
                ))
                .child(
                    theme.toggle_group().children(
                        [
                            (icons::glyph::SquareKanban, "Lanes"),
                            (icons::glyph::LayoutList, "List"),
                        ]
                        .into_iter()
                        .enumerate()
                        .map(|(index, (glyph, label))| {
                            pressable(
                                focus::focusable(
                                    &theme,
                                    &self.controls.segments_view[index],
                                    theme.toggle_group_icon(
                                        glyph,
                                        self.controls.segment_view == index,
                                    ),
                                ),
                                SharedString::from(format!("segment-view-{index}")),
                                cx,
                                move |view, cx| {
                                    view.controls.segment_view = index;
                                    cx.notify();
                                },
                            )
                            .tooltip(move |window, cx| Tooltip::text(label, window, cx))
                            .into_any_element()
                        }),
                    ),
                )
                .into_any_element(),

            "collapsible" => {
                let open = self.controls.details.get(self.controls.running);
                section
                    .child(
                        div()
                            .w_full()
                            .max_w(px(320.0))
                            .child(
                                div()
                                    .id("collapse")
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.controls.expanded = !view.controls.expanded;
                                        cx.notify();
                                    }))
                                    .child(
                                        theme
                                            .collapsible_header("Advanced", self.controls.expanded)
                                            .hover(|s| s.bg(theme.element_hover)),
                                    ),
                            )
                            .when(self.controls.expanded, |el| {
                                el.child(
                                    div()
                                        .pl(px(24.0))
                                        .pt(px(4.0))
                                        .text_style(TextStyle::Callout)
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
                                        view.controls.running = !view.controls.running;
                                        cx.notify();
                                    }))
                                    .child(theme.button(
                                        if self.controls.running {
                                            "Finish the run"
                                        } else {
                                            "Start a run"
                                        },
                                        ButtonStyle::Ghost,
                                        Some(Fade::new(view, "g-takeover-run")),
                                    )),
                            )
                            // Which of the two rules is answering, on the page —
                            // the same trick the follow-scroll row uses. A
                            // behaviour you can only infer is one nobody checks.
                            .child(
                                div()
                                    .text_style(TextStyle::Callout)
                                    .font_family(theme.font_mono.clone())
                                    .text_color(theme.text_faint)
                                    .child(SharedString::from(format!(
                                        "open: {open} — {}",
                                        if self.controls.details == widgets::Takeover::default() {
                                            "following the run"
                                        } else {
                                            "yours"
                                        }
                                    ))),
                            ),
                    )
                    .child(
                        div()
                            .w_full()
                            .max_w(px(320.0))
                            .child(
                                div()
                                    .id("takeover-head")
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        let running = view.controls.running;
                                        view.controls.details.toggle(running);
                                        cx.notify();
                                    }))
                                    .child(
                                        theme
                                            .collapsible_header(
                                                if self.controls.running {
                                                    "Working"
                                                } else {
                                                    "Details"
                                                },
                                                open,
                                            )
                                            .hover(|s| s.bg(theme.element_hover)),
                                    ),
                            )
                            .when(open, |el| {
                                el.child(
                                    div()
                                        .ml(px(10.0))
                                        .pl(px(12.0))
                                        .border_l_1()
                                        .border_color(theme.border)
                                        .text_style(TextStyle::Callout)
                                        .text_color(theme.text_muted)
                                        .child(if self.controls.running {
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

            _ => return None,
        })
    }
}

impl Gallery {
    /// Every button face, the groupings they gather into, and the ghost frame
    /// a caller fills itself.
    pub(crate) fn buttons(&mut self, cx: &mut Context<Self>) -> Vec<(&'static str, AnyElement)> {
        let theme = Theme::of(cx).clone();
        let view = Painter::of(cx);
        let labels = ["Ghost", "Prominent", "Destructive"];
        let faces = [
            theme.button(
                labels[0],
                ButtonStyle::Ghost,
                Some(Fade::new(view, "g-ghost")),
            ),
            theme.button(labels[1], ButtonStyle::Prominent, None),
            theme.button(labels[2], ButtonStyle::Destructive, None),
        ];
        let glyphs = [
            (icons::glyph::Pen, ButtonStyle::Ghost, "pen"),
            (icons::glyph::Plus, ButtonStyle::Prominent, "plus"),
            (icons::glyph::Trash, ButtonStyle::Destructive, "trash"),
        ];
        let toolbar = [
            (icons::glyph::PanelLeft, "sidebar"),
            (icons::glyph::Search, "search"),
            (icons::glyph::Settings, "settings"),
        ];
        let cluster = theme
            .control_group()
            .children(toolbar.into_iter().map(|(glyph, name)| {
                theme
                    .icon_button(glyph, ButtonStyle::Ghost, Some(Fade::new(view, name)))
                    .id(name)
                    .on_click(cx.listener(move |view, _, _, cx| view.press(name, cx)))
                    .into_any_element()
            }));
        let texts = theme
            .control_group()
            .children(["Cut", "Copy", "Paste"].into_iter().map(|label| {
                theme
                    .button(label, ButtonStyle::Ghost, Some(Fade::new(view, label)))
                    .control_size(ControlSize::Small)
                    .id(label)
                    .on_click(cx.listener(move |view, _, _, cx| view.press(label, cx)))
                    .into_any_element()
            }));
        let capsule = theme.control_group().rounded_full().children(
            [
                (icons::glyph::ChevronLeft, "back"),
                (icons::glyph::ChevronRight, "forward"),
            ]
            .into_iter()
            .map(|(glyph, name)| {
                theme
                    .icon_button(glyph, ButtonStyle::Ghost, Some(Fade::new(view, name)))
                    .rounded_full()
                    .id(name)
                    .on_click(cx.listener(move |view, _, _, cx| view.press(name, cx)))
                    .into_any_element()
            }),
        );
        let lensed = row().children(toolbar.into_iter().map(|(glyph, name)| {
            theme
                .icon_button(
                    glyph,
                    ButtonStyle::Ghost,
                    Some(Fade::new(view, format!("lens-{name}"))),
                )
                .rounded_full()
                .id(SharedString::from(format!("lens-{name}")))
                .on_click(cx.listener(move |view, _, _, cx| view.press(name, cx)))
                .surface(&theme, theme.popover_surface)
                .into_any_element()
        }));
        let split = theme
            .control_group()
            .child(
                theme
                    .button("Save", ButtonStyle::Prominent, None)
                    .id("group-save")
                    .on_click(cx.listener(|view, _, _, cx| view.press("Save", cx))),
            )
            .child(
                theme
                    .icon_button(
                        icons::glyph::ChevronDown,
                        ButtonStyle::Ghost,
                        Some(Fade::new(view, "group-more")),
                    )
                    .id("group-more")
                    .on_click(cx.listener(|view, _, _, cx| view.press("more", cx))),
            );

        let basic = stack()
            .child(
                row()
                    .child(
                        ui::widgets::Button::new("semantic-save", "Save")
                            .button_style(ButtonStyle::Prominent)
                            .on_press(cx.listener(|view, _, _, cx| view.press("Save", cx))),
                    )
                    .child(
                        ui::widgets::Button::new("semantic-delete", "Delete")
                            .role(ui::widgets::ButtonRole::Destructive)
                            .icon(icons::glyph::Trash)
                            .on_press(cx.listener(|view, _, _, cx| view.press("Delete", cx))),
                    )
                    .child(
                        ui::widgets::Button::new("semantic-disabled", "Unavailable")
                            .enabled(false)
                            .on_press(cx.listener(|view, _, _, cx| view.press("Unavailable", cx))),
                    ),
            )
            .when_some(self.last_pressed.clone(), |page, label| {
                page.child(
                    div()
                        .text_style(TextStyle::Callout)
                        .text_color(theme.text_muted)
                        .child(SharedString::from(format!("pressed: {label}"))),
                )
            });

        let styles = row().children(faces.into_iter().enumerate().map(|(index, face)| {
            pressable(
                focus::focusable(&theme, &self.controls.buttons[index], face),
                SharedString::from(format!("button-{index}")),
                cx,
                move |view, cx| view.press(labels[index], cx),
            )
            .into_any_element()
        }));

        let icon = row().children(glyphs.into_iter().enumerate().map(
            |(index, (glyph, style, name))| {
                pressable(
                    focus::focusable(
                        &theme,
                        &self.controls.icon_buttons[index],
                        theme.icon_button(glyph, style, None),
                    ),
                    SharedString::from(format!("icon-button-{index}")),
                    cx,
                    move |view, cx| view.press(name, cx),
                )
                .into_any_element()
            },
        ));

        let ghost = row()
            .child(
                theme
                    .ghost("ghost-menu")
                    .p(px(5.0))
                    .child(
                        icons::icon(icons::glyph::Ellipsis)
                            .size(px(14.0))
                            .text_color(theme.text_faint),
                    )
                    .on_click(cx.listener(|view, _, _, cx| view.press("menu", cx))),
            )
            .child(
                theme
                    .ghost("ghost-new")
                    .px(px(8.0))
                    .py(px(4.0))
                    .gap(px(6.0))
                    .text_style(TextStyle::Callout)
                    .text_color(theme.text_muted)
                    .child(
                        icons::icon(icons::glyph::Plus)
                            .size(px(13.0))
                            .text_color(theme.text_faint),
                    )
                    .child("New")
                    .on_click(cx.listener(|view, _, _, cx| view.press("New", cx))),
            );

        vec![
            ("basic", basic.into_any_element()),
            ("styles", styles.into_any_element()),
            ("icon", icon.into_any_element()),
            (
                "group",
                stack()
                    .child(row().child(cluster).child(split))
                    .child(row().child(texts))
                    .into_any_element(),
            ),
            ("capsule", row().child(capsule).into_any_element()),
            ("lensed", lensed.into_any_element()),
            ("ghost", ghost.into_any_element()),
        ]
    }
}

/// What this group's demos hold between frames.
pub(crate) struct State {
    pub(crate) search: Entity<TextField>,
    pub(crate) filled: Entity<TextField>,
    pub(crate) segment: usize,
    /// Which glyph segment the icon-only toggle group is on.
    pub(crate) segment_view: usize,
    pub(crate) expanded: bool,
    /// The second collapsible: a section that follows a run until you take it
    /// over. `running` is what a streaming flag would be in a real app.
    pub(crate) running: bool,
    pub(crate) details: widgets::Takeover,
    /// Select state lives here, not in a component: the menu is mounted by
    /// this view, so this view owns whether it is open and what is chosen.
    pub(crate) theme_menu: popover::Popup<()>,
    pub(crate) theme_choice: usize,
    /// The combobox, by contrast, owns its own menu — it has a query field to
    /// hold, so it is an entity.
    pub(crate) language: Entity<Combobox>,
    /// Focus for the wired controls. A stateless `fn(&Theme, ..) -> Div` has
    /// nowhere to keep a handle, so the view that composes it holds them —
    /// the same place it already holds what each one is set to.
    pub(crate) buttons: [gpui::FocusHandle; 3],
    pub(crate) icon_buttons: [gpui::FocusHandle; 3],
    pub(crate) checkboxes: [gpui::FocusHandle; 2],
    pub(crate) radios: [gpui::FocusHandle; 2],
    pub(crate) switches: [gpui::FocusHandle; 2],
    pub(crate) segments: [gpui::FocusHandle; 3],
    pub(crate) segments_view: [gpui::FocusHandle; 2],
    pub(crate) slider: gpui::FocusHandle,
    /// What the wired controls are set to. Every one of them paints from the
    /// caller's state and reports nothing back, so this is where the answer is.
    pub(crate) checked: [bool; 2],
    pub(crate) radio: usize,
    pub(crate) switched: [bool; 2],
    pub(crate) level: f32,
}

impl State {
    pub(crate) fn new(cx: &mut Context<Gallery>) -> Self {
        Self {
            search: cx.new(|cx| TextField::new(cx).with_placeholder("Search components…")),
            filled: cx.new(|cx| {
                let mut field = TextField::new(cx);
                field.set_content("Select me with shift-left", cx);
                field
            }),
            segment: 0,
            segment_view: 0,
            expanded: true,
            // Arrives mid-run, which is the state the auto-follow is for.
            running: true,
            details: widgets::Takeover::default(),
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
            buttons: [cx.focus_handle(), cx.focus_handle(), cx.focus_handle()],
            icon_buttons: [cx.focus_handle(), cx.focus_handle(), cx.focus_handle()],
            checkboxes: [cx.focus_handle(), cx.focus_handle()],
            radios: [cx.focus_handle(), cx.focus_handle()],
            switches: [cx.focus_handle(), cx.focus_handle()],
            segments: [cx.focus_handle(), cx.focus_handle(), cx.focus_handle()],
            segments_view: [cx.focus_handle(), cx.focus_handle()],
            slider: cx.focus_handle(),
            checked: [true, false],
            radio: 0,
            switched: [true, false],
            level: 0.5,
        }
    }
}
