use crate::*;

impl Gallery {
    pub(crate) fn overlays(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let theme = Theme::of(cx).clone();
        let view = Painter::of(cx);
        let section = stack();

        Some(match key {
            "group-box" => section
                .child(
                    theme
                        .group_box()
                        .child(
                            theme
                                .card_row(true)
                                .hover(|s| s.bg(theme.element_hover))
                                .child(theme.row_icon(icons::glyph::Monitor))
                                .child(theme.row_title("First row")),
                        )
                        .child(
                            theme
                                .card_row(false)
                                .hover(|s| s.bg(theme.element_hover))
                                .child(theme.row_icon(icons::glyph::Folder))
                                .child(theme.row_title("Second row")),
                        ),
                )
                .into_any_element(),

            "empty-state" => section
                .child(theme.group_box().child(theme.empty_state(
                    icons::glyph::Folder,
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
                                        .text_style(TextStyle::Caption)
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

            "stats" => {
                // Showing the page is showing the meter: it moves out of the
                // window's corner and into the column here, so the page
                // documents a component you are looking at rather than one it
                // describes. One instance either way — two of them would each
                // count the other's frames.
                if !self.stats_shown {
                    self.show_stats(true, cx);
                }
                section
                    .child(hint(
                        &theme,
                        "This is the meter, measuring the window it is sitting in. \
                         At rest it reads 0 — nothing on screen is asking for a \
                         frame, and its own two-a-second tick is the one draw it \
                         does not count. Mount the spinner and read what one \
                         animation costs a whole window: every frame it asks for is \
                         a full redraw, and the number is the rate it is really \
                         getting.",
                    ))
                    .child(self.stats.clone())
                    .child(
                        row()
                            .child(
                                theme
                                    .toggle(self.stats_spinner)
                                    .id("stats-spinner")
                                    .cursor_pointer()
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.stats_spinner = !view.stats_spinner;
                                        cx.notify();
                                    })),
                            )
                            .child(div().child("Mount a spinner"))
                            .when(self.stats_spinner, |row| {
                                row.child(loaders::pulse_loader(
                                    "stats-pulse",
                                    &theme,
                                    8.0,
                                    view,
                                    cx,
                                ))
                            }),
                    )
                    .child(hint(
                        &theme,
                        "CPU is the whole process — user plus system, every thread — \
                         as a percentage of one core, and MEM is that same \
                         process's resident memory: the two figures Activity \
                         Monitor prints. A browser tab can see neither of its own \
                         process, so both read — on the web build.",
                    ))
                    .child(hint(
                        &theme,
                        "While the meter is up this app keeps animating when you \
                         switch away from it, so the numbers carry on where they \
                         would otherwise freeze. That is the cost being measured, \
                         paid on purpose: switch the meter off and the app parks \
                         itself in the background again.",
                    ))
                    .into_any_element()
            }

            "floating" => section
                .child(hint(
                    &theme,
                    "Grab the card and move it. The panel lays a layer over the \
                     surface it is mounted in and places the box inside it, so a \
                     pointer that outruns a frame is still heard and the box never \
                     stalls behind the cursor. It clamps nothing: dragged half off \
                     the edge it stays there, and the point you grabbed it by is \
                     under the pointer to drag it back.",
                ))
                .child(
                    div()
                        .relative()
                        .h(px(280.0))
                        .w_full()
                        .rounded(px(Theme::panel_radius()))
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.surface)
                        .overflow_hidden()
                        .child(floating::panel(
                            "floating-demo",
                            &self.panel_demo,
                            gpui::point(px(40.0), px(40.0)),
                            popover::popover_card(&theme)
                                .w(px(180.0))
                                .child(popover::menu_heading(&theme, "Drag me"))
                                .surface(&theme, theme.popover_surface),
                        )),
                )
                .child(hint(
                    &theme,
                    "A layer is the panel without the drag: a band the app places \
                     over its own page, taking the presses and the wheel that land \
                     on it. Hitboxes in gpui are paint-order only, so a plain \
                     absolute box hands both to whatever sits behind it — press the \
                     band below, then the rows, and watch which one answers.",
                ))
                .child(
                    div()
                        .relative()
                        .h(px(160.0))
                        .w_full()
                        .rounded(px(Theme::panel_radius()))
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.surface)
                        .overflow_hidden()
                        .child(
                            div()
                                .id("layer-page")
                                .size_full()
                                .flex()
                                .flex_col()
                                .children((0..6).map(|row| {
                                    div()
                                        .id(SharedString::from(format!("layer-row-{row}")))
                                        .w_full()
                                        .px(px(12.0))
                                        .py(px(8.0))
                                        .text_color(theme.text_muted)
                                        .hover(|el| el.bg(theme.element_hover))
                                        .on_click(cx.listener(move |view: &mut Self, _, _, cx| {
                                            view.layer_answer = Some(format!("row {row}").into());
                                            cx.notify();
                                        }))
                                        .child(SharedString::from(format!("Row {row}")))
                                })),
                        )
                        .child(
                            floating::layer("layer-band")
                                .bottom_0()
                                .left_0()
                                .right_0()
                                .h(px(56.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .on_click(cx.listener(|view: &mut Self, _, _, cx| {
                                    view.layer_answer = Some("the band".into());
                                    cx.notify();
                                }))
                                .child(
                                    popover::popover_card(&theme)
                                        .px(px(16.0))
                                        .child(match &self.layer_answer {
                                            Some(what) => {
                                                SharedString::from(format!("{what} took the press"))
                                            }
                                            None => SharedString::from("Press the band"),
                                        })
                                        .surface(&theme, theme.popover_surface),
                                ),
                        ),
                )
                .into_any_element(),

            "palette" => section
                .child(
                    row()
                        // Read off the keymap, not typed here: this page and
                        // `init` are two files, and a chord written in both is
                        // a chord that drifts.
                        .child(popover::key_hint_text(
                            &theme,
                            keys::shortcut(&OpenPalette, window).unwrap_or_default(),
                            "open palette",
                        ))
                        .when_some(self.last_command.clone(), |r, cmd| {
                            r.child(
                                div()
                                    .text_style(TextStyle::Body)
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
                    row().children(
                        [
                            ("open-sheet", "From the side", popover::Side::Right),
                            (
                                "open-bottom-sheet",
                                "From the bottom",
                                popover::Side::Bottom,
                            ),
                        ]
                        .into_iter()
                        .map(|(id, label, side)| {
                            div()
                                .id(id)
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.sheet.open(side);
                                    cx.notify();
                                }))
                                .child(theme.button(
                                    label,
                                    ButtonStyle::Ghost,
                                    Some(Fade::new(view, format!("g-{id}"))),
                                ))
                                .into_any_element()
                        }),
                    ),
                )
                .into_any_element(),

            "context-menu" => section
                .child(hint(&theme, "Right-click anywhere in this window."))
                .child(
                    div()
                        .h(px(120.0))
                        .w_full()
                        .rounded(px(Theme::panel_radius()))
                        .border_1()
                        .border_color(theme.border)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_style(TextStyle::Callout)
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
                                Some(Fade::new(view, "g-dialog")),
                            )),
                    ),
                )
                .into_any_element(),

            _ => return None,
        })
    }
}
