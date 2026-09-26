//! The window around a section: nav, header and the body frame.

use crate::*;

impl Gallery {
    /// The navigation rail: every component, one row each, the current one
    /// carrying the same selected wash a menu row does — this is the library
    /// browsing itself.
    /// The top nav: the kind of thing you are browsing, and the appearance
    /// switch. Everything here is global — per-page detail belongs in
    /// [`Self::header`].
    pub(crate) fn nav(
        &self,
        theme: &Theme,
        compact: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let current = self.tab;
        let dark = matches!(theme.appearance, theme::Appearance::Dark);
        // This strip *is* the window's titlebar — the traffic lights are
        // painted over it, and off macOS it carries the caption cluster, the
        // menus and the grip the window is moved by. `false` for the traffic
        // lights because the padding below already clears them.
        titlebar::titlebar("gallery-nav", false, window)
            .flex_none()
            .h(px(Theme::HEADER_HEIGHT))
            .pl(px(if compact {
                COMPACT_NAV_PAD - NAV_ITEM_PAD
            } else {
                RAIL_WIDTH + CARD_PAD - NAV_ITEM_PAD
            }))
            .pr(px(if APP_MENUBAR { 0.0 } else { CARD_PAD }))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(18.0))
            // Wherever the desktop puts them. Empty on macOS, and empty on the
            // side a GNOME layout leaves bare.
            .when(APP_MENUBAR, |strip| {
                strip
                    .child(titlebar::controls(titlebar::CaptionSide::Left, window, cx))
                    // What `cx.set_menus` mounts in the system bar on macOS.
                    .child(self.app_menus.clone())
            })
            .when(compact, |strip| {
                strip.child(
                    div()
                        .id("drawer-toggle")
                        .p(px(NAV_ITEM_PAD))
                        .cursor_pointer()
                        .on_click(cx.listener(|view, _, _, cx| view.open_drawer(cx)))
                        .child(
                            icons::icon(icons::glyph::PanelLeft)
                                .size(px(15.0))
                                .text_color(theme.text_muted),
                        ),
                )
            })
            // Takes the free space, which is what pushes the trailing controls
            // to the far edge — and once there is none left the tabs scroll
            // under them rather than shoving them off the window.
            .child(
                div()
                    .id("nav-tabs")
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(18.0))
                    .when(compact, |strip| scroll::scrolls(strip, Axes::Horizontal))
                    .children(TABS.iter().enumerate().map(|(index, tab)| {
                        let selected = index == current;
                        let mut item = div()
                            .id(SharedString::from(format!("nav-{}", tab.title)))
                            .flex_none()
                            .p(px(NAV_ITEM_PAD))
                            .text_style(TextStyle::Body)
                            .cursor_pointer()
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.open(index, view.selected[index], cx);
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
                    // The stretch past the last tab is what moves the window,
                    // inside the strip rather than beside it so a compact window
                    // gives it up to the tabs instead of to a gap.
                    .when(APP_MENUBAR, |strip| {
                        strip.child(titlebar::grip("nav-grip", &self.drag, window))
                    }),
            )
            // The frame meter, on any page rather than only the one that
            // documents it: what a window costs is a property of what you are
            // looking at, so it has to follow you around to be worth reading.
            // Except where it cannot be read — the panel it opens is dragged,
            // and a drag on a touch screen is a pan.
            .when(!compact, |strip| {
                strip.child(
                    div()
                        .id("stats-toggle")
                        .p(px(4.0))
                        .cursor_pointer()
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.show_stats(!view.stats_shown, cx);
                        }))
                        .child(icons::icon(icons::glyph::Cpu).size(px(15.0)).text_color(
                            if self.stats_shown {
                                theme.text
                            } else {
                                theme.text_faint
                            },
                        )),
                )
            })
            // One button carrying the appearance it is already in, not three
            // segments and not a switch: the glyph says which of two you are
            // looking at, so a track and a second icon only say it again.
            //
            // It reads the *resolved* appearance rather than the mode, so it
            // shows what is actually on screen while the app is still following
            // the OS — and the first press is what pins it. Returning to
            // `System` is `set_mode`, a settings-level action rather than a
            // nav-level one.
            .child(
                div()
                    .id("appearance")
                    .p(px(NAV_ITEM_PAD))
                    .cursor_pointer()
                    .on_click(cx.listener(move |view, _, _, cx| {
                        appearance::set_mode(
                            if dark {
                                AppearanceMode::Light
                            } else {
                                AppearanceMode::Dark
                            },
                            cx,
                        );
                        // The probe's knobs are a look's numbers, and the two
                        // appearances do not share them: carrying dark's over
                        // paints light with dark's material and reads as the
                        // theme being broken.
                        view.material.probe_spec = view.probe_look(cx);
                        cx.notify();
                    }))
                    .child(
                        icons::icon(if dark {
                            icons::glyph::Moon
                        } else {
                            icons::glyph::Sun
                        })
                        .size(px(15.0))
                        .text_color(theme.text_muted),
                    ),
            )
            .when(APP_MENUBAR, |strip| {
                strip.child(titlebar::controls(titlebar::CaptionSide::Right, window, cx))
            })
            .into_any_element()
    }

    /// The bar over the pane: what you are looking at, and where it is written.
    pub(crate) fn header(&self, section: &'static Section, theme: &Theme) -> AnyElement {
        div()
            .flex_none()
            .h(px(Theme::HEADER_HEIGHT))
            .px(px(CARD_PAD))
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
                            .text_style(TextStyle::Title3)
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
                        .text_style(TextStyle::Subheadline)
                        .text_color(theme.text_faint),
                    ),
            )
            .into_any_element()
    }

    /// The section's demos, keyed by [`Example::key`]. A section still written
    /// as one undivided body answers with a single unkeyed element.
    pub(crate) fn examples(
        &mut self,
        section: &'static Section,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<(&'static str, AnyElement)> {
        match section.key {
            "buttons" => self.buttons(cx),
            _ => vec![("", self.section_body(section.key, window, cx))],
        }
    }

    /// The example keys this section actually paints. The catalog declares
    /// them and [`Self::examples`] produces them; comparing the two is what
    /// keeps a doc page's `example=` from pointing at nothing.
    pub fn painted(
        &mut self,
        section: &'static Section,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<&'static str> {
        self.examples(section, window, cx)
            .into_iter()
            .map(|(key, _)| key)
            .collect()
    }

    /// What the pane paints: every example stacked under its title, or — when
    /// a doc page has pointed the embed at one — that example alone, with no
    /// caption, since the page beside it is already the heading.
    ///
    /// An example key the section does not carry widens back to the whole page
    /// rather than emptying the pane, so a stale doc link still shows the
    /// component.
    pub(crate) fn body(
        &mut self,
        section: &'static Section,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::of(cx).clone();
        let mut examples = self.examples(section, window, cx);
        if let Some(key) = self.example.clone()
            && let Some(index) = examples.iter().position(|(k, _)| *k == key.as_ref())
        {
            return examples.swap_remove(index).1;
        }
        if examples.len() == 1 {
            return examples.remove(0).1;
        }
        stack()
            .children(examples.into_iter().map(|(key, element)| {
                let title = section
                    .examples
                    .iter()
                    .find(|example| example.key == key)
                    .map(|example| example.title);
                stack()
                    .when_some(title, |column, title| {
                        column.child(popover::menu_heading(&theme, title))
                    })
                    .child(element)
                    .into_any_element()
            }))
            .into_any_element()
    }
}

impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = Painter::of(cx);
        let theme = Theme::of(cx).clone();
        let reduce_motion = cx.reduce_motion();
        // gpui has no media query: every responsive decision in this file is
        // this one read, and it is the window rather than the element — the
        // same thing a CSS breakpoint measures.
        let compact = window.viewport_size().width < px(COMPACT_BELOW);
        // A rail that is showing beside the pane must not also be in the
        // drawer: one entity, two mounts. Widening the window is the case.
        if !compact {
            self.drawer.close();
        }

        // Rail on the left, one component in the pane — the set is long enough
        // that a single scroll of everything reads as a wall.
        let section =
            section_at(self.selected[self.tab]).unwrap_or(&TABS[self.tab].groups[0].sections[0]);
        let body = self.body(section, window, cx);
        let pane = div().relative().flex_1().min_h_0().map(|pane| {
            if TABS[self.tab].full_bleed {
                // A pattern is a screen: it takes the pane whole and
                // scrolls its own parts, so neither the fixed column nor
                // the pane's own scrollbar applies to it.
                //
                // Narrower than a screen it stays one anyway, and you pan
                // across it — a pattern is a file you copy into a desktop app,
                // and thirteen phone layouts of one document nothing.
                pane.child(
                    div()
                        .id("gallery-canvas")
                        .size_full()
                        .when(compact, |canvas| scroll::scrolls(canvas, Axes::Both))
                        .child(
                            div()
                                .size_full()
                                .when(compact, |screen| screen.min_w(px(CANVAS_MIN)))
                                .p(px(24.0))
                                .child(body),
                        ),
                )
            } else {
                pane.child(
                    scroll::pane("gallery-pane", Axes::Vertical)
                        .size_full()
                        .track_scroll(&self.pane_scroll)
                        // The column width components are designed for;
                        // several are `w_full` and would otherwise stretch
                        // to the whole pane.
                        .child(
                            div()
                                // Compact drops to the header's padding, so the
                                // body and the section title share one grid.
                                .p(px(if compact { CARD_PAD } else { PANE_PAD }))
                                .child(column().child(body)),
                        ),
                )
                .child(scroll::transient(
                    "pane-bar",
                    &self.pane_scroll,
                    &self.pane_bar,
                    reduce_motion,
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
                .child(self.nav(&theme, compact, window, cx))
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .flex_row()
                        .when(!compact, |row| {
                            row.child(self.rail.clone().cached(rail::style()))
                        })
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .h_full()
                                .flex()
                                .flex_col()
                                .bg(theme.surface)
                                .overflow_hidden()
                                // Runs off the window's right and bottom edges,
                                // so the only corner that floats is the one
                                // that gets rounded. The seam it rounds against
                                // is the rail, so a drawer leaves it nothing to
                                // round and the pane meets the window edge.
                                .when(!compact, |card| {
                                    card.rounded_tl(px(Theme::panel_radius()))
                                        .border_t_1()
                                        .border_l_1()
                                        .border_color(theme.border)
                                })
                                .child(self.header(section, &theme))
                                .child(pane),
                        ),
                )
        };

        // Traversal goes on the root so `tab` works wherever focus happens to
        // be, rather than only inside whatever claimed it.
        let root = focus::traversal(div())
            .id("gallery-scroll")
            .key_context("Gallery")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::open_palette))
            .on_action(cx.listener(Self::toggle_fps_overlay))
            .on_action(cx.listener(Self::reset_frame_overlay_stats))
            .on_action(cx.listener(Self::toggle_full_screen))
            .on_action(cx.listener(Self::close_overlay))
            .on_mouse_down(
                gpui::MouseButton::Right,
                cx.listener(|view, event: &gpui::MouseDownEvent, _, cx| {
                    view.context_menu.open(event.position);
                    cx.notify();
                }),
            )
            .size_full()
            .bg(theme.window_bg())
            .font_family(theme.font_sans.clone())
            .text_color(theme.text)
            .text_style(TextStyle::Body)
            .child(content)
            .child(
                div()
                    .absolute()
                    .bottom(px(CARD_PAD))
                    .right(px(CARD_PAD))
                    .child(
                        self.pending
                            .get_or_insert_with(|| cx.new(|cx| PendingKeys::new(window, cx)))
                            .clone(),
                    ),
            )
            // The rail, once it no longer fits beside the pane. Same width, so
            // the cached layout `rail::style` reports still describes it.
            .when(self.drawer.get().is_some(), |root| {
                root.child(popover::sheet(
                    "gallery-drawer",
                    window.viewport_size(),
                    popover::Side::Left,
                    px(RAIL_WIDTH),
                    popover::sheet_panel(&theme, popover::Side::Left)
                        // Starts where the rail starts in the wide layout —
                        // below the nav strip. The scrim dims that strip but
                        // cannot dim the traffic lights over it, which AppKit
                        // paints above the canvas.
                        .pt(px(Theme::HEADER_HEIGHT))
                        .child(self.rail.clone().cached(rail::style()))
                        .into_any_element(),
                    self.drawer.closing_since(),
                    cx.listener(|view, _, _, cx| view.close_drawer(cx)),
                ))
            })
            // Not on the page that documents it: that page mounts the meter in
            // its own column, and the entity can only be in one place. Nor on a
            // narrow window, where its home corner is most of the width.
            .when(
                self.stats_shown && !compact && section.key != "stats",
                |root| {
                    // Home is read off the viewport rather than stored, so the
                    // corner it opens in is the corner of *this* window.
                    let viewport = window.viewport_size();
                    let home = gpui::point(
                        viewport.width - px(stats::WIDTH + CARD_PAD),
                        px(Theme::HEADER_HEIGHT + CARD_PAD),
                    );
                    root.child(floating::panel(
                        "meter",
                        &self.stats_at,
                        home,
                        self.stats.clone(),
                    ))
                },
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
                                        Some(Fade::new(view, format!("ctx-{index}"))),
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
                                            Some(Fade::new(view, "g-dialog-no")),
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
            .when_some(self.sheet.get().copied(), |root, side| {
                // A side sheet is as wide as a rail; a bottom one is as tall
                // as a picker, which is a different number for the same reason
                // — it is measured across the edge it hangs off.
                let extent = match side {
                    popover::Side::Bottom => px(300.0),
                    _ => px(320.0),
                };
                root.child(popover::sheet(
                    "gallery-sheet",
                    window.viewport_size(),
                    side,
                    extent,
                    popover::sheet_panel(&theme, side)
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
                                            Some(Fade::new(view, "g-sheet-close")),
                                        )),
                                ),
                        )
                        .child(popover::dialog_body(
                            &theme,
                            "A sheet is the dialog card pinned to an edge — same scrim, \
                             same glass, spanning whichever edge it hangs off.",
                        ))
                        .child(
                            theme
                                .group_box()
                                .child(
                                    theme
                                        .card_row(true)
                                        .hover(|s| s.bg(theme.element_hover))
                                        .child(theme.row_icon(icons::glyph::Monitor))
                                        .child(theme.row_title("Appearance")),
                                )
                                .child(
                                    theme
                                        .card_row(false)
                                        .hover(|s| s.bg(theme.element_hover))
                                        .child(theme.row_icon(icons::glyph::Folder))
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
            });

        // Border, corners, shadow and resize edges, for the desktops that hand
        // the window over undecorated. Everywhere else this is the root back
        // unchanged.
        ui::window::frame(root, window, cx)
    }
}
