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
                        view.probe_spec = view.probe_look(cx);
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
                focus::focusable(&theme, &self.buttons[index], face),
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
                        &self.icon_buttons[index],
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

    /// Open the tab strip's `at`, wrapping at both ends, and take the focus
    /// with it.
    pub(crate) fn open_tab(&mut self, at: isize, window: &mut Window, cx: &mut Context<Self>) {
        let count = self.tab_strip.len() as isize;
        let at = at.rem_euclid(count) as usize;
        self.tab_choice = at;
        window.focus(&self.tab_strip[at], cx);
        cx.notify();
    }

    /// Put `held` in front of `before` — where the drift demo's drop lands.
    /// The list is the app's, the way a board's cards are: bezel reports where
    /// the pointer let go and arranges nothing itself.
    pub(crate) fn move_chip(
        &mut self,
        held: &SharedString,
        before: &SharedString,
        cx: &mut Context<Self>,
    ) {
        if held == before {
            return;
        }
        let Some(from) = self.drift_chips.iter().position(|chip| chip == held) else {
            return;
        };
        let chip = self.drift_chips.remove(from);
        let at = self
            .drift_chips
            .iter()
            .position(|chip| chip == before)
            .unwrap_or(self.drift_chips.len());
        self.drift_chips.insert(at, chip);
        cx.notify();
    }
}
