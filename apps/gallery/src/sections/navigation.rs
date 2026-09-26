use crate::*;

impl Gallery {
    pub(crate) fn navigation(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let theme = Theme::of(cx).clone();
        let view = Painter::of(cx);
        let section = stack();

        Some(match key {
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
                                Tooltip::with_keystroke(
                                    "Copy path",
                                    keys::printed("secondary-c"),
                                    window,
                                    cx,
                                )
                            })
                            .child(theme.button(
                                "Hover me",
                                ButtonStyle::Ghost,
                                Some(Fade::new(view, "g-tip")),
                            )),
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
                            .child(theme.tag("@clearloop")),
                    ),
                )
                .into_any_element(),

            "tabs" => section
                .child(hint(
                    &theme,
                    "Sections of one page, not things that open and close — see \
                     Tab strip for those. Tab walks the strip, space or enter \
                     opens the focused tab, and ← / → move the selection and \
                     carry the focus with it.",
                ))
                .child(
                    theme.tab_bar().children(
                        ["Components", "Tokens", "Motion"]
                            .into_iter()
                            .enumerate()
                            .map(|(index, label)| {
                                pressable(
                                    focus::focusable(
                                        &theme,
                                        &self.tab_strip[index],
                                        theme.tab(label, self.tab_choice == index),
                                    ),
                                    SharedString::from(format!("tab-{index}")),
                                    cx,
                                    move |view, cx| {
                                        view.tab_choice = index;
                                        cx.notify();
                                    },
                                )
                                // bezel binds ← / → to `Decrement`/`Increment`
                                // for a focused control holding a *value*, and
                                // a strip's value is which tab is open. The
                                // focus goes with the selection, or the next
                                // arrow starts from the tab you left.
                                .on_action(cx.listener(
                                    move |view, _: &focus::Decrement, window, cx| {
                                        view.open_tab(index as isize - 1, window, cx);
                                    },
                                ))
                                .on_action(cx.listener(
                                    move |view, _: &focus::Increment, window, cx| {
                                        view.open_tab(index as isize + 1, window, cx);
                                    },
                                ))
                                .into_any_element()
                            }),
                    ),
                )
                .into_any_element(),

            "tab-strip" => section
                .child(hint(
                    &theme,
                    "Open things rather than sections: each tab closes, the front \
                     one carries the wash, and the strip scrolls sideways once the \
                     tabs stop fitting. The order and the front are a \
                     `tabs::Strip`; what a tab opens is the app's.",
                ))
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            tabs::bar("demo-strip").children(self.strip.tabs().iter().map(
                                |open| {
                                    let key = *open;
                                    let front = self.strip.active() == Some(&key);
                                    let mut label = tabs::Label::new(key);
                                    if let Some((_, icon, dirty, badge)) =
                                        STRIP_TABS.iter().find(|(name, ..)| *name == key)
                                    {
                                        label = label.with_icon(*icon);
                                        if *dirty {
                                            label =
                                                label.mark(icons::Icon::glyph(STRIP_MARK).solid());
                                        }
                                        if !badge.is_empty() {
                                            label = label.with_badge(*badge);
                                        }
                                    }
                                    tabs::tab(
                                        &theme,
                                        key,
                                        label,
                                        match front {
                                            true => tabs::State::Focused,
                                            false => tabs::State::Resting,
                                        },
                                    )
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        view.strip.activate(&key);
                                        cx.notify();
                                    }))
                                    .child(
                                        tabs::close(&theme, key, tabs::Close::OnHover).on_click(
                                            cx.listener(move |view, _, _, cx| {
                                                cx.stop_propagation();
                                                view.strip.close(&key);
                                                cx.notify();
                                            }),
                                        ),
                                    )
                                },
                            )),
                        )
                        .child(
                            theme
                                .ghost("strip-add")
                                .flex_none()
                                .p(px(4.0))
                                .tooltip(|window, cx| Tooltip::text("Open a tab", window, cx))
                                .child(
                                    icons::icon(icons::glyph::Plus)
                                        .size(px(14.0))
                                        .text_color(theme.text_muted),
                                )
                                .on_click(cx.listener(|view, _, _, cx| {
                                    if let Some((next, ..)) = STRIP_TABS
                                        .iter()
                                        .find(|(name, ..)| !view.strip.contains(name))
                                    {
                                        view.strip.open(next);
                                        cx.notify();
                                    }
                                })),
                        )
                        .child(div().flex_1())
                        .children([-1isize, 1].map(|step| {
                            theme
                                .ghost(SharedString::from(format!("strip-cycle-{step}")))
                                .flex_none()
                                .p(px(4.0))
                                .tooltip(move |window, cx| {
                                    Tooltip::text(
                                        match step {
                                            1 => "Next tab",
                                            _ => "Previous tab",
                                        },
                                        window,
                                        cx,
                                    )
                                })
                                .child(
                                    icons::icon(match step {
                                        1 => icons::glyph::ChevronRight,
                                        _ => icons::glyph::ChevronLeft,
                                    })
                                    .size(px(14.0))
                                    .text_color(theme.text_muted),
                                )
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.strip.cycle(step);
                                    cx.notify();
                                }))
                        })),
                )
                .child(
                    div()
                        .w_full()
                        .h(px(120.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(Theme::panel_radius()))
                        .border_1()
                        .border_color(theme.border)
                        .text_style(TextStyle::Callout)
                        .text_color(theme.text_muted)
                        .child(match self.strip.active() {
                            Some(open) => SharedString::from(format!("{open} is in front")),
                            None => SharedString::from("Nothing open"),
                        }),
                )
                .into_any_element(),

            "nav-row" => {
                const ROWS: [(&[u8], &str); 5] = [
                    (icons::glyph::LayoutGrid, "Home"),
                    (icons::glyph::Globe, "Browser"),
                    (icons::glyph::Book, "Articles"),
                    (icons::glyph::Archive, "Archived"),
                    (icons::glyph::FileText, "Untitled"),
                ];
                section
                    .child(hint(
                        &theme,
                        "The sidebar row. Trailing content is the caller's — a \
                         count and a chevron here, and a control that appears \
                         on hover, painted off the row's own fade key because \
                         gpui allows one hover listener per element.",
                    ))
                    .child(
                        div()
                            .w(px(240.0))
                            .p(px(6.0))
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .rounded(px(Theme::panel_radius()))
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.surface)
                            .children(ROWS.iter().enumerate().map(|(index, (icon, label))| {
                                let key = Fade::new(view, format!("nav-row-{index}"));
                                let row = theme
                                    .nav_row(
                                        Some(Icon::glyph(icon)),
                                        *label,
                                        self.nav_choice == index,
                                        key.clone(),
                                    )
                                    .when(*label == "Archived", |row| {
                                        row.child(
                                            div()
                                                .flex_none()
                                                .flex()
                                                .items_center()
                                                .gap(px(4.0))
                                                .text_style(TextStyle::Subheadline)
                                                .text_color(theme.text_faint)
                                                .child("10")
                                                .child(
                                                    icons::icon(icons::glyph::ChevronRight)
                                                        .size(px(14.0))
                                                        .text_color(theme.text_faint),
                                                ),
                                        )
                                    })
                                    .when(*label == "Untitled", |row| {
                                        row.child(
                                            icons::icon(icons::glyph::Trash)
                                                .size(px(14.0))
                                                .flex_none()
                                                .text_color(motion::hover_blend(
                                                    &key,
                                                    theme.text_muted.opacity(0.0),
                                                    theme.text_muted,
                                                )),
                                        )
                                    });
                                pressable(row, key.key.clone(), cx, move |view, cx| {
                                    view.nav_choice = index;
                                    cx.notify();
                                })
                                .into_any_element()
                            })),
                    )
                    .into_any_element()
            }

            "titlebar" => {
                let frame = |body: gpui::Div| {
                    body.w_full()
                        .rounded(px(Theme::panel_radius()))
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.surface)
                        .overflow_hidden()
                };
                let caption = |copy: &'static str| {
                    div()
                        .flex_1()
                        .text_style(TextStyle::Callout)
                        .text_color(theme.text_muted)
                        .child(copy)
                };
                section
                    .child(hint(
                        &theme,
                        "Drag the bare stretch of either strip to move the \
                         window; double-click it to zoom. That stretch is a \
                         grip, and it is the only part that drags — which is \
                         what leaves the button beside it its own click. The \
                         move starts on the first motion after the press.",
                    ))
                    .child(
                        frame(div()).child(
                            titlebar::titlebar("titlebar-lights", true, window)
                                .pr(px(8.0))
                                .child(caption("Traffic lights cleared"))
                                .child(titlebar::grip(
                                    "titlebar-lights-grip",
                                    &self.titlebar_drag,
                                    window,
                                ))
                                .child(pressable(
                                    {
                                        let hover = theme.element_hover;
                                        control_bar::bar_button(
                                            icons::glyph::Search,
                                            24.0,
                                            theme.text_muted,
                                        )
                                        .hover(move |s| s.bg(hover))
                                    },
                                    "titlebar-search",
                                    cx,
                                    |view, cx| view.press("Search", cx),
                                )),
                        ),
                    )
                    .child(
                        frame(div()).child(
                            titlebar::titlebar("titlebar-plain", false, window)
                                .px(px(8.0))
                                .child(caption("A pane with no lights over it"))
                                .child(titlebar::grip(
                                    "titlebar-plain-grip",
                                    &self.titlebar_drag,
                                    window,
                                )),
                        ),
                    )
                    .when_some(self.last_pressed.clone(), |page, label| {
                        page.child(
                            div()
                                .text_style(TextStyle::Callout)
                                .text_color(theme.text_muted)
                                .child(SharedString::from(format!("pressed: {label}"))),
                        )
                    })
                    .into_any_element()
            }

            "split" => {
                let hint = hint(&theme, "Drag the divider; it clamps at 15% either side.");
                let muted = theme.text_muted;
                let pane = move |label: SharedString| {
                    div()
                        .h_full()
                        .p(px(12.0))
                        .text_style(TextStyle::Callout)
                        .text_color(muted)
                        .child(label)
                };
                section
                    .child(hint)
                    .child(
                        div()
                            .id("split")
                            .w_full()
                            .max_w(px(420.0))
                            .h(px(140.0))
                            .rounded(px(Theme::panel_radius()))
                            .border_1()
                            .border_color(theme.border)
                            .overflow_hidden()
                            .flex()
                            .flex_row()
                            .on_drag_move(cx.listener(
                                |view, event: &DragMoveEvent<SplitDrag>, _, cx| {
                                    view.split = widgets::axis_fraction(
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
                                theme
                                    .split_handle(
                                        Axis::Horizontal,
                                        SplitStyle::Line {
                                            dragging: self.split_dragging,
                                        },
                                    )
                                    .id("split-handle")
                                    .on_drag(SplitDrag, |_, _, _, cx| cx.new(|_| Empty)),
                            )
                            .child(div().flex_1().child(pane("drag the divider".into()))),
                    )
                    .into_any_element()
            }

            "control-bar" => {
                let glyph = |name: &'static str, icon: &'static [u8]| {
                    let hover = theme.element_hover;
                    ui::control_bar::bar_button(icon, 30.0, theme.text_muted)
                        .id(name)
                        .hover(move |s| s.bg(hover))
                };
                let label = |copy: &'static str| {
                    div()
                        .text_style(TextStyle::Callout)
                        .text_color(theme.text_muted)
                        .child(copy)
                };
                section
                    .child(hint(
                        &theme,
                        "One bar, three jobs, two shapes. The centre is centred on \
                         the BAR rather than on what the clusters leave — five \
                         controls on the left and one on the right still put it \
                         on axis.",
                    ))
                    .child(theme.field_label("Transport — Shape::Pill"))
                    .child(ui::control_bar::control_bar(
                        &theme,
                        ControlBarShape::Pill,
                        vec![
                            glyph("shuffle", icons::glyph::Shuffle).into_any_element(),
                            glyph("skip-back", icons::glyph::SkipBack).into_any_element(),
                            glyph("play", icons::glyph::Play).into_any_element(),
                            glyph("skip-forward", icons::glyph::SkipForward).into_any_element(),
                            glyph("repeat", icons::glyph::Repeat).into_any_element(),
                        ],
                        Some(label("Grain").into_any_element()),
                        vec![glyph("volume-2", icons::glyph::Volume2).into_any_element()],
                    ))
                    // Rounded, not a stadium: a composer is not a media control,
                    // and the stadium reads as one.
                    .child(theme.field_label("Composer — Shape::Rounded"))
                    .child(ui::control_bar::control_bar(
                        &theme,
                        ControlBarShape::Rounded,
                        vec![glyph("plus", icons::glyph::Plus).into_any_element()],
                        Some(label("Ask anything…").into_any_element()),
                        vec![
                            glyph("mic", icons::glyph::Mic).into_any_element(),
                            glyph("arrow-up", icons::glyph::ArrowUp).into_any_element(),
                        ],
                    ))
                    .child(theme.field_label("Floating over content"))
                    // The same striped band the materials page uses, and the only
                    // place either shape's blur can be caught disagreeing with
                    // its border.
                    .child(
                        div()
                            .relative()
                            .h(px(130.0))
                            .rounded(px(Theme::panel_radius()))
                            .overflow_hidden()
                            .child(div().absolute().inset_0().flex().flex_row().children(
                                (0..14).map(|i| {
                                    div().flex_1().h_full().bg(if i % 2 == 0 {
                                        theme.accent
                                    } else {
                                        theme.warning
                                    })
                                }),
                            ))
                            .child(
                                div()
                                    .absolute()
                                    .bottom(px(16.0))
                                    .left_0()
                                    .right_0()
                                    .flex()
                                    .justify_center()
                                    .child(ui::control_bar::control_bar(
                                        &theme,
                                        ControlBarShape::Pill,
                                        vec![
                                            glyph("panel-left", icons::glyph::PanelLeft)
                                                .into_any_element(),
                                            glyph("search", icons::glyph::Search)
                                                .into_any_element(),
                                        ],
                                        None,
                                        vec![
                                            glyph(
                                                "sliders-horizontal",
                                                icons::glyph::SlidersHorizontal,
                                            )
                                            .into_any_element(),
                                            glyph("settings", icons::glyph::Settings)
                                                .into_any_element(),
                                        ],
                                    )),
                            ),
                    )
                    .into_any_element()
            }

            "menu" => {
                // The card reads `menu_style` to know whether it owes a fill, a
                // hairline and a shadow — so demoing a look the app has not
                // chosen means handing it a theme that has, the same rule the
                // probe tunes glass by.
                let card = |style: SurfaceStyle, tag: &'static str| {
                    let theme = Theme {
                        popover_surface: style,
                        ..theme.clone()
                    };
                    popover::popover_card(&theme).w(px(240.0)).children([
                        popover::menu_heading(&theme, "Section").into_any_element(),
                        popover::menu_row(&theme, false, Some(Fade::new(view, tag)))
                            .child("First item")
                            .into_any_element(),
                        popover::menu_row(&theme, true, Some(Fade::new(view, "m-active")))
                            .child("Active item")
                            .into_any_element(),
                        popover::divider().into_any_element(),
                        popover::menu_row(&theme, false, Some(Fade::new(view, "m-third")))
                            .child("Third item")
                            .into_any_element(),
                    ])
                };
                // Over content, not the page: a menu on a flat backdrop cannot
                // tell the two looks apart, because the one thing separating
                // them is what they do to what is behind.
                let band =
                    |style: SurfaceStyle, tag: &'static str| {
                        div()
                            .relative()
                            .h(px(280.0))
                            .rounded(px(Theme::panel_radius()))
                            .overflow_hidden()
                            .child(div().absolute().inset_0().flex().flex_row().children(
                                (0..14).map(|i| {
                                    div().flex_1().h_full().bg(if i % 2 == 0 {
                                        theme.accent
                                    } else {
                                        theme.warning
                                    })
                                }),
                            ))
                            .child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(ui::surface::of(
                                        Theme::surface_radius(),
                                        style,
                                        card(style, tag),
                                    )),
                            )
                    };
                section
                    .child(hint(
                        &theme,
                        "The same menu on both looks. Material washes what it \
                         covers; glass dims it and bends it at the rim. \
                         `Theme::menu_style` picks which one every menu, dialog \
                         and sheet in the app mounts on.",
                    ))
                    .child(theme.field_label("Material — Regular"))
                    .child(band(SurfaceStyle::Material(Material::Regular), "m-frosted"))
                    .child(theme.field_label("Glass — Regular"))
                    .child(band(SurfaceStyle::Glass(Glass::Regular), "m-regular"))
                    .into_any_element()
            }

            _ => return None,
        })
    }
}
