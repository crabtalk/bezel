use crate::*;

impl Gallery {
    pub(crate) fn foundations(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let theme = Theme::of(cx).clone();
        let section = stack();

        Some(match key {
            // ---- Foundations -------------------------------------------------
            "theme" => brand::page(self, &theme, window, cx),

            "color" => section
                .child(hint(
                    &theme,
                    "Tokens read at paint time from the theme global — the appearance \
                     button above swaps every one of them.",
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
                    "Geist and Geist Mono ship with the crate. Every size below is \
                     a role on the system ladder, measured off \
                     NSFont.preferredFont(forTextStyle:) — components name the \
                     role, never a number.",
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
                                .text_style(TextStyle::Title3)
                                .font_weight(weight)
                                .child(SharedString::from(label))
                                .into_any_element()
                        }),
                    ),
                )
                .child(popover::menu_heading(&theme, "Scale"))
                .child({
                    let (floor, ceiling) = BASE_TEXT_RANGE;
                    let span = ceiling - floor;
                    let base = theme::base_text_size();
                    let nudge = move |cx: &mut Context<Self>, by: f32| {
                        let next = (theme::base_text_size() + by).clamp(floor, ceiling);
                        theme::set_base_text_size(next, cx);
                        cx.notify();
                    };
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(12.0))
                        .child(
                            div()
                                .w(px(88.0))
                                .flex_none()
                                .text_style(TextStyle::Subheadline)
                                .text_color(theme.text_muted)
                                .child("Body"),
                        )
                        .child(
                            div().w(px(240.0)).flex_none().child(
                                focus::focusable(
                                    &theme,
                                    &self.type_probe,
                                    theme.slider((base - floor) / span),
                                )
                                .id("type-probe")
                                .on_drag(SliderDrag("type-probe".into()), |_, _, _, cx| {
                                    cx.new(|_| Empty)
                                })
                                .on_drag_move(cx.listener(
                                    move |_, event: &DragMoveEvent<SliderDrag>, _, cx| {
                                        let Some(fraction) =
                                            widgets::slider_fraction(event, "type-probe", cx)
                                        else {
                                            return;
                                        };
                                        // Whole points: every stop is a size
                                        // you could write into the ladder.
                                        let points = (floor + fraction * span).round();
                                        theme::set_base_text_size(points, cx);
                                        cx.notify();
                                    },
                                ))
                                .on_action(cx.listener(move |_, _: &focus::Decrement, _, cx| {
                                    nudge(cx, -1.0)
                                }))
                                .on_action(
                                    cx.listener(move |_, _: &focus::Increment, _, cx| {
                                        nudge(cx, 1.0)
                                    }),
                                ),
                            ),
                        )
                        .child(
                            div()
                                .w(px(56.0))
                                .flex_none()
                                .text_style(TextStyle::Subheadline)
                                .font_family(theme.font_mono.clone())
                                .text_color(theme.text_faint)
                                .child(SharedString::from(format!("{base:.0}"))),
                        )
                })
                .child(stack().children(TYPE_SCALE.iter().map(|style| {
                    div()
                        .flex()
                        .flex_row()
                        .items_baseline()
                        .gap(px(12.0))
                        .child(
                            div()
                                .w(px(88.0))
                                .flex_none()
                                .text_style(TextStyle::Subheadline)
                                .font_family(theme.font_mono.clone())
                                .text_color(theme.text_muted)
                                .child(SharedString::from(format!("{style:?}"))),
                        )
                        .child(
                            div()
                                .w(px(56.0))
                                .flex_none()
                                .text_style(TextStyle::Subheadline)
                                .font_family(theme.font_mono.clone())
                                .text_color(theme.text_faint)
                                .child(SharedString::from(format!(
                                    "{} · {}",
                                    style.size() * theme::base_text_size() / TextStyle::Body.size(),
                                    style.weight().0
                                ))),
                        )
                        .child(div().text_style(*style).child("The quick brown fox jumps"))
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
                            ("SPACE", Theme::SPACE),
                            ("CONTENT_MARGIN", Theme::CONTENT_MARGIN),
                        ]
                        .into_iter()
                        .map(|(name, value)| measure(&theme, name, value).into_any_element()),
                    ),
                )
                .child(popover::menu_heading(&theme, "Radius"))
                .child(
                    div().flex().flex_row().gap(px(12.0)).children(
                        [
                            ("CONTROL", Theme::control_radius()),
                            ("PANEL", Theme::panel_radius()),
                            ("BUBBLE", Theme::bubble_radius()),
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
                                        .text_style(TextStyle::Subheadline)
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
                            ("EASE", motion::EASE),
                            ("EASE_OUT", motion::EASE_OUT),
                            ("EASE_OUT_EXPO", motion::EASE_OUT_EXPO),
                            ("EASE_IN_OUT", motion::EASE_IN_OUT),
                            ("EASE_RESORT", motion::EASE_RESORT),
                            ("EASE_TAILWIND", motion::EASE_TAILWIND),
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
                                .text_style(TextStyle::Callout)
                                .font_family(theme.font_mono.clone())
                                .child(SharedString::from(*name)),
                        )
                        .child(
                            div()
                                .w(px(70.0))
                                .flex_none()
                                .text_style(TextStyle::Subheadline)
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

            "icons" => {
                // A sample, not the set. All 1834 are there to name; painting
                // every one of them here would say nothing the naming rule does
                // not, and would pin the whole set into this binary.
                const SAMPLE: [(&str, &[u8]); 24] = [
                    ("search", icons::glyph::Search),
                    ("house", icons::glyph::House),
                    ("settings", icons::glyph::Settings),
                    ("user", icons::glyph::User),
                    ("bell", icons::glyph::Bell),
                    ("calendar", icons::glyph::Calendar),
                    ("folder", icons::glyph::Folder),
                    ("file-text", icons::glyph::FileText),
                    ("image", icons::glyph::Image),
                    ("mail", icons::glyph::Mail),
                    ("message-circle", icons::glyph::MessageCircle),
                    ("heart", icons::glyph::Heart),
                    ("star", icons::glyph::Star),
                    ("play", icons::glyph::Play),
                    ("volume-2", icons::glyph::Volume2),
                    ("wifi", icons::glyph::Wifi),
                    ("battery", icons::glyph::Battery),
                    ("cpu", icons::glyph::Cpu),
                    ("terminal", icons::glyph::Terminal),
                    ("git-branch", icons::glyph::GitBranch),
                    ("shopping-cart", icons::glyph::ShoppingCart),
                    ("map-pin", icons::glyph::MapPin),
                    ("sun", icons::glyph::Sun),
                    ("moon", icons::glyph::Moon),
                ];

                let rule = |from: &'static str, to: &'static str| {
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .font_family(theme.font_mono.clone())
                        .text_style(TextStyle::Caption)
                        .child(div().text_color(theme.text_faint).child(from))
                        .child(div().text_color(theme.text_faint).child("→"))
                        .child(div().text_color(theme.text).child(to))
                };

                section
                    .child(hint(
                        &theme,
                        "The whole Lucide set, ported from a pinned release into typed \
                         constants. Browse the drawings at lucide.dev — the name there is \
                         the name here, so nothing has to be looked up twice.",
                    ))
                    .child(theme.field_label("Naming"))
                    .child(
                        stack()
                            .gap(px(6.0))
                            .child(rule("arrow-big-down", "icons::arrows::ArrowBigDown"))
                            .child(rule(
                                "message-circle",
                                "icons::communication::MessageCircle",
                            ))
                            .child(rule("volume-2", "icons::multimedia::Volume2")),
                    )
                    .child(hint(
                        &theme,
                        "The module is the category lucide.dev files it under, and a glyph \
                         filed under two is reachable through either. `icons::glyph` holds \
                         every one of them if the category is not worth remembering.",
                    ))
                    .child(theme.field_label("Painting one"))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(14.0))
                            .child(theme.icon(icons::glyph::Star))
                            .child(theme.icon(Icon::glyph(icons::glyph::Star).solid()))
                            .child(theme.icon_at(TextStyle::Title2, icons::glyph::Star))
                            .child(theme.icon(icons::glyph::Star).text_color(theme.accent)),
                    )
                    .child(hint(
                        &theme,
                        "`theme.icon` takes its size from the type ladder and its tone from \
                         the palette, so an icon set nowhere still paints — a `Styled` call \
                         after it wins, which is how a component keeps its own metric. \
                         `Icon::solid` is the filled variant, `theme.icon_at` another rung, \
                         and `Icon::path` art the app resolves itself.",
                    ))
                    .child(theme.field_label("Cargo"))
                    .child(markdown::render(
                        &self.icons_cargo,
                        markdown::Caption::Hidden,
                        window,
                        cx,
                    ))
                    .child(hint(
                        &theme,
                        "One feature per category, none on by default, `full` for all 42. \
                         A constant is the SVG itself, so within an enabled category the \
                         linker still drops whatever the binary never names.",
                    ))
                    .child(theme.field_label("A sample"))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap(px(8.0))
                            .children(SAMPLE.map(|(name, glyph)| {
                                div()
                                    .w(px(96.0))
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .gap(px(6.0))
                                    .py(px(10.0))
                                    .rounded(px(Theme::control_radius()))
                                    .border_1()
                                    .border_color(theme.border)
                                    .child(
                                        icons::icon(glyph)
                                            .size(px(18.0))
                                            .text_color(theme.text_muted),
                                    )
                                    .child(
                                        div()
                                            .w_full()
                                            .truncate()
                                            .text_style(TextStyle::Caption)
                                            .text_align(gpui::TextAlign::Center)
                                            .font_family(theme.font_mono.clone())
                                            .text_color(theme.text_faint)
                                            .child(name),
                                    )
                            })),
                    )
                    .into_any_element()
            }

            _ => return None,
        })
    }
}
