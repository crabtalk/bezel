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
                            popover::menu_trigger(
                                div().id("theme-select"),
                                |view: &mut Self| &mut view.theme_menu,
                                |_| (),
                                cx,
                            )
                            .child(theme.select_trigger(SELECT_CHOICES[self.theme_choice]))
                            .when(menu_open, |trigger| {
                                trigger.child(popover::anchored_menu_below(
                                    "theme-select-menu",
                                    // Dismissal is the caller's, and the
                                    // caller is this view — without it,
                                    // clicking away leaves it open.
                                    popover::dismiss_on_out(
                                        popover::popover_card(&theme).w(px(200.0)),
                                        |view: &mut Self| &mut view.theme_menu,
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
                                            .when(index == self.theme_choice, |row| {
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
                        .text_style(TextStyle::Callout)
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
                            .on_drag(SliderDrag("slider".into()), |_, _, _, cx| cx.new(|_| Empty))
                            .on_drag_move(cx.listener(
                                |view, event: &DragMoveEvent<SliderDrag>, _, cx| {
                                    let Some(fraction) =
                                        widgets::slider_fraction(event, "slider", cx)
                                    else {
                                        return;
                                    };
                                    view.level = fraction;
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
                        .text_style(TextStyle::Callout)
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
                                    &self.segments_view[index],
                                    theme.toggle_group_icon(glyph, self.segment_view == index),
                                ),
                                SharedString::from(format!("segment-view-{index}")),
                                cx,
                                move |view, cx| {
                                    view.segment_view = index;
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
                let open = self.details.get(self.running);
                section
                    .child(
                        div()
                            .w_full()
                            .max_w(px(320.0))
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
                                            .hover(|s| s.bg(theme.element_hover)),
                                    ),
                            )
                            .when(self.expanded, |el| {
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
                            .w_full()
                            .max_w(px(320.0))
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

            _ => return None,
        })
    }
}
