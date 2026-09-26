use crate::*;

impl Gallery {
    pub(crate) fn material(
        &mut self,
        key: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let theme = Theme::of(cx).clone();
        let section = stack();

        Some(match key {
            // Exercises the fork's backdrop-blur primitive. The backdrop is
            // big curved edges and text on a dark field, not a tiling pattern:
            // a lens reads as a *named* edge bending, and a tile whose period
            // matches the bevel just maps onto itself.
            "material" => {
                // The probe: drag the glass over a backdrop, resize it live.
                // Mirrors the SwiftUI reference harness so the two can be put
                // side by side on identical content.
                let bg_names = [
                    "flat", "bars", "h-ruler", "v-ruler", "gradient", "text", "coded",
                ];
                let probe_bg =
                    |i: usize| {
                        let base = div().absolute().inset_0().overflow_hidden();
                        match i {
                            0 => base.bg(gpui::hsla(0.0, 0.0, 0.5, 1.0)),
                            1 => {
                                base.bg(gpui::hsla(0.0, 0.0, 0.5, 1.0)).child(
                                    div().absolute().inset_0().flex().flex_row().children(
                                        (0..6).map(|j| {
                                            div().flex_1().h_full().bg(match j {
                                                0 => theme.text_muted,
                                                1 => theme.warning,
                                                2 => theme.danger,
                                                3 => theme.success,
                                                4 => theme.accent,
                                                _ => theme.warning_muted,
                                            })
                                        }),
                                    ),
                                )
                            }
                            2 => base
                                .bg(gpui::hsla(0.0, 0.0, 0.5, 1.0))
                                .children((0..30).map(|k| {
                                    div()
                                        .absolute()
                                        .top(px(k as f32 * 12.0))
                                        .left_0()
                                        .right_0()
                                        .h(px(2.0))
                                        .bg(gpui::hsla(0.0, 0.0, 0.0, 0.85))
                                })),
                            3 => base
                                .bg(gpui::hsla(0.0, 0.0, 0.5, 1.0))
                                .children((0..70).map(|k| {
                                    div()
                                        .absolute()
                                        .left(px(k as f32 * 12.0))
                                        .top_0()
                                        .bottom_0()
                                        .w(px(2.0))
                                        .bg(gpui::hsla(0.0, 0.0, 0.0, 0.85))
                                })),
                            4 => {
                                base.bg(theme.accent).child(
                                    div().absolute().inset_0().flex().flex_row().children(
                                        (0..40).map(|k| {
                                            div()
                                                .flex_1()
                                                .h_full()
                                                .bg(theme.warning.opacity(k as f32 / 39.0))
                                        }),
                                    ),
                                )
                            }
                            // A position code: green ramps once across the
                            // probe, red sawtooths every 32pt. A pixel under the
                            // glass names the backdrop position it came from, so
                            // the displacement is read rather than inferred.
                            6 => base.children((0..420).map(|k| {
                                let x = k as f32 + 0.5;
                                let chan = |v: f32| (20.0 + 180.0 * v) / 255.0;
                                div()
                                    .absolute()
                                    .left(px(k as f32))
                                    .top_0()
                                    .bottom_0()
                                    .w(px(1.0))
                                    .bg(gpui::Rgba {
                                        r: chan((x % 32.0) / 32.0),
                                        g: chan(x / 420.0),
                                        b: 0.0,
                                        a: 1.0,
                                    })
                            })),
                            // What glass actually sits on in an app. Letterforms
                            // are the honest probe: a displacement shows up as
                            // text you can no longer read straight.
                            _ => base.bg(theme.bg).children((0..16).map(|k| {
                                div()
                                    .absolute()
                                    .top(px(8.0 + k as f32 * 21.0))
                                    .left(px(12.0))
                                    .right(px(12.0))
                                    .text_style(TextStyle::Title3)
                                    .text_color(theme.text)
                                    .whitespace_nowrap()
                                    .overflow_hidden()
                                    .child(
                                        "the quick brown fox jumps over the lazy dog \
                                     and back again",
                                    )
                            })),
                        }
                    };
                // The sliders tune a *theme*, not the card. Glass numbers flow
                // from the environment, so the honest way to demonstrate them
                // is to hand this one card a different environment — which is
                // exactly what branding the lens would do.
                // The chips load a look's numbers and the knobs move them from
                // there, so what the probe paints is always `probe_spec` — one
                // conduit, whichever style named the numbers in it.
                let probe_theme = Theme {
                    glass_regular: self.material.probe_spec,
                    glass_magnify: self.material.probe_magnify * 32.0 - 16.0,
                    glass_dispersion: self.material.probe_disp,
                    ..theme.clone()
                };
                let pw = 120.0 + self.material.probe_w * 480.0;
                let ph = 30.0 + self.material.probe_h * 220.0;
                let pr = self.material.probe_r * (pw.min(ph) / 2.0);
                let chip = |i: usize, on: bool| {
                    div()
                        .id(bg_names[i])
                        .px(px(10.0))
                        .py(px(4.0))
                        .rounded(px(Theme::control_radius()))
                        .text_style(TextStyle::Callout)
                        .text_color(if on { theme.text } else { theme.text_muted })
                        .bg(if on { theme.element_hover } else { theme.bg })
                        .cursor_pointer()
                        .child(bg_names[i])
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.material.probe_bg = i;
                            cx.notify();
                        }))
                };
                // `value` and `range` are the knob's own units — points, a
                // gain, an alpha — so a label is the number to write back into
                // the palette, and only the slider sees a fraction.
                let knob =
                    |id: &'static str, slot: usize, value: f32, range: f32, label: SharedString| {
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .w(px(56.0))
                                    .text_style(TextStyle::Subheadline)
                                    .text_color(theme.text_muted)
                                    .child(label),
                            )
                            .child(
                                // Tab reaches it and the arrows step it, which is
                                // the only way to set a number a drag cannot hit.
                                // `probe-m` spans -16..+16, so the sweep passes
                                // through zero and the lens inverts halfway.
                                focus::focusable(
                                    &theme,
                                    &self.material.probe_knobs[slot],
                                    div().w(px(130.0)).child(theme.slider(value / range)),
                                )
                                .id(id)
                                .on_drag(SliderDrag(id.into()), |_, _, _, cx| cx.new(|_| Empty))
                                .on_drag_move(cx.listener(
                                    move |view, event: &DragMoveEvent<SliderDrag>, _, cx| {
                                        let Some(f) = widgets::slider_fraction(event, id, cx)
                                        else {
                                            return;
                                        };
                                        *view.probe_knob(id) = f * range;
                                        cx.notify();
                                    },
                                ))
                                .on_action(cx.listener(move |view, _: &focus::Decrement, _, cx| {
                                    let knob = view.probe_knob(id);
                                    *knob = (*knob - PROBE_STEP * range).clamp(0.0, range);
                                    cx.notify();
                                }))
                                .on_action(cx.listener(
                                    move |view, _: &focus::Increment, _, cx| {
                                        let knob = view.probe_knob(id);
                                        *knob = (*knob + PROBE_STEP * range).clamp(0.0, range);
                                        cx.notify();
                                    },
                                )),
                            )
                    };
                let probe = div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .relative()
                            .h(px(340.0))
                            .w_full()
                            .rounded(px(Theme::panel_radius()))
                            .overflow_hidden()
                            .child(probe_bg(self.material.probe_bg))
                            .child(floating::panel(
                                "glass-probe",
                                &self.material.probe_at,
                                point(px(120.0), px(120.0)),
                                {
                                    let glass = div()
                                        .w(px(pw))
                                        .h(px(ph))
                                        .rounded(px(pr))
                                        .surface(&probe_theme, SurfaceStyle::Glass(Glass::Regular));
                                    if self.material.probe_tint {
                                        glass
                                            .tint(
                                                gpui::hsla(0.58, 0.85, 0.55, 1.0)
                                                    .opacity(self.material.probe_fill),
                                            )
                                            .into_any_element()
                                    } else {
                                        glass.into_any_element()
                                    }
                                },
                            )),
                    )
                    // What is behind the glass, on its own row: mixed in with
                    // the mode chips a seventh backdrop wraps and reads as one.
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_3()
                            .flex_wrap()
                            .children((0..7).map(|i| chip(i, self.material.probe_bg == i))),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_3()
                            .flex_wrap()
                            .children(PROBE_STYLES.map(|style| {
                                let on = self.material.probe_style == style;
                                div()
                                    .id(probe_style_name(style))
                                    .px(px(10.0))
                                    .py(px(4.0))
                                    .rounded(px(Theme::control_radius()))
                                    .text_style(TextStyle::Callout)
                                    .text_color(if on { theme.text } else { theme.text_muted })
                                    .bg(if on { theme.element_hover } else { theme.bg })
                                    .cursor_pointer()
                                    .child(probe_style_name(style))
                                    // Land on the look's shipped numbers, so
                                    // the sliders explore from the truth
                                    // rather than overriding it silently.
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        view.material.probe_style = style;
                                        view.material.probe_spec = view.probe_look(cx);
                                        cx.notify();
                                    }))
                            }))
                            .child(
                                div()
                                    .id("probe-tint")
                                    .px(px(10.0))
                                    .py(px(4.0))
                                    .rounded(px(Theme::control_radius()))
                                    .text_style(TextStyle::Callout)
                                    .text_color(if self.material.probe_tint {
                                        theme.text
                                    } else {
                                        theme.text_muted
                                    })
                                    .bg(if self.material.probe_tint {
                                        theme.element_hover
                                    } else {
                                        theme.bg
                                    })
                                    .cursor_pointer()
                                    .child("tint")
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.material.probe_tint = !view.material.probe_tint;
                                        cx.notify();
                                    })),
                            )
                            // The corner is the one place a second window can be
                            // put in exactly, which is what makes two probes
                            // comparable without dragging either.
                            .child(
                                div()
                                    .id("probe-reset")
                                    .px(px(10.0))
                                    .py(px(4.0))
                                    .rounded(px(Theme::control_radius()))
                                    .text_style(TextStyle::Callout)
                                    .text_color(theme.text_muted)
                                    .bg(theme.bg)
                                    .cursor_pointer()
                                    .child("reset")
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.material.probe_at.move_to(point(px(0.0), px(0.0)));
                                        cx.notify();
                                    })),
                            )
                            .child(knob(
                                "probe-w",
                                0,
                                self.material.probe_w,
                                1.0,
                                format!("w {}", pw as i32).into(),
                            ))
                            .child(knob(
                                "probe-h",
                                1,
                                self.material.probe_h,
                                1.0,
                                format!("h {}", ph as i32).into(),
                            ))
                            .child(knob(
                                "probe-r",
                                2,
                                self.material.probe_r,
                                1.0,
                                format!("r {}", pr as i32).into(),
                            ))
                            .child(knob(
                                "probe-blur",
                                6,
                                self.material.probe_spec.blur,
                                BLUR_RANGE,
                                format!("blur {:.1}pt", self.material.probe_spec.blur).into(),
                            ))
                            .child(knob(
                                "probe-b",
                                3,
                                self.material.probe_spec.rim,
                                RIM_RANGE,
                                format!("rim {:.1}pt", self.material.probe_spec.rim).into(),
                            ))
                            // The one knob still a fraction: it spans -16..+16,
                            // so the sweep passes through zero and inverts.
                            .child(knob(
                                "probe-m",
                                4,
                                self.material.probe_magnify,
                                1.0,
                                format!("mag {:+.1}", self.material.probe_magnify * 32.0 - 16.0)
                                    .into(),
                            ))
                            .child(knob(
                                "probe-dim",
                                5,
                                self.material.probe_spec.gain,
                                GAIN_RANGE,
                                format!("gain {:.3}", self.material.probe_spec.gain).into(),
                            ))
                            // A gain alone moves level and colour
                            // together; this is what takes the level down
                            // and leaves the colours where they were.
                            .child(knob(
                                "probe-sat",
                                12,
                                self.material.probe_spec.saturation,
                                SAT_RANGE,
                                format!("sat {:.2}", self.material.probe_spec.saturation).into(),
                            ))
                            .child(knob(
                                "probe-lift",
                                8,
                                self.material.probe_spec.tint.a,
                                LIFT_RANGE,
                                format!("lift {:.0}/255", self.material.probe_spec.tint.a * 255.0)
                                    .into(),
                            ))
                            .child(knob(
                                "probe-edge",
                                9,
                                self.material.probe_spec.edge,
                                EDGE_RANGE,
                                format!("edge {:.2}", self.material.probe_spec.edge).into(),
                            ))
                            .child(knob(
                                "probe-edgew",
                                10,
                                self.material.probe_spec.edge_width,
                                EDGE_W_RANGE,
                                format!("edge w {:.1}pt", self.material.probe_spec.edge_width)
                                    .into(),
                            ))
                            .child(knob(
                                "probe-disp",
                                11,
                                self.material.probe_disp,
                                DISPERSION_RANGE,
                                format!("disp {:.3}", self.material.probe_disp).into(),
                            ))
                            // Only a tinted surface reads this: everywhere else
                            // the look's own tint is the fill.
                            .when(self.material.probe_tint, |row| {
                                row.child(knob(
                                    "probe-fill",
                                    7,
                                    self.material.probe_fill,
                                    1.0,
                                    format!("fill {:.2}", self.material.probe_fill).into(),
                                ))
                            }),
                    );

                section
                    .child(hint(
                        &theme,
                        "Drag the glass, resize it, switch what is behind it. \
                         Materialed blurs what it covers; glass lifts it and lenses \
                         at the rim. Both read very differently depending on the \
                         backdrop, which is what the switcher is for.",
                    ))
                    .when(!ui::surface::lensed(&theme), |el| {
                        el.child(theme.warning_strip(
                            "Liquid glass is macOS only — the lens is a Metal \
                             primitive. Here it falls back to the tone the look \
                             settles on: the card and its shape, opaque, without \
                             the refraction at the rim.",
                        ))
                    })
                    .child(probe)
                    .into_any_element()
            }

            _ => return None,
        })
    }
}

impl Gallery {
    /// The slider by keyboard. Clamped here rather than in the widget: the
    /// paint clamps what it draws, but the value is this view's to keep sane.
    /// The shipped look the probe's variant chip is pointing at.
    pub(crate) fn probe_look(&self, cx: &App) -> SurfaceSpec {
        let theme = Theme::of(cx);
        self.material.probe_style.spec(theme)
    }

    /// The probe knob a slider id drives. Drag writes its value, the arrows add
    /// a step; both go through here so the two cannot disagree.
    pub(crate) fn probe_knob(&mut self, id: &str) -> &mut f32 {
        match id {
            "probe-w" => &mut self.material.probe_w,
            "probe-h" => &mut self.material.probe_h,
            "probe-b" => &mut self.material.probe_spec.rim,
            "probe-m" => &mut self.material.probe_magnify,
            "probe-dim" => &mut self.material.probe_spec.gain,
            "probe-sat" => &mut self.material.probe_spec.saturation,
            "probe-blur" => &mut self.material.probe_spec.blur,
            "probe-lift" => &mut self.material.probe_spec.tint.a,
            "probe-edge" => &mut self.material.probe_spec.edge,
            "probe-edgew" => &mut self.material.probe_spec.edge_width,
            "probe-disp" => &mut self.material.probe_disp,
            "probe-fill" => &mut self.material.probe_fill,
            _ => &mut self.material.probe_r,
        }
    }
}

/// What this group's demos hold between frames.
pub(crate) struct State {
    pub(crate) probe_knobs: [gpui::FocusHandle; 13],
    /// The glass probe: where it has been dragged, its size and rounding as
    /// slider fractions, and which backdrop it is over. Sizes are fractions
    /// because that is what a slider reports.
    pub(crate) probe_at: Floating,
    pub(crate) probe_w: f32,
    pub(crate) probe_h: f32,
    pub(crate) probe_r: f32,
    pub(crate) probe_bg: usize,
    /// Which look the probe paints — one three-way, since frost is a look.
    pub(crate) probe_style: SurfaceStyle,
    /// The look under the knobs, in the units it ships in — a spec rather than
    /// six fractions, so seating it on a shipped look is an assignment and the
    /// labels are the numbers to write back into the palette.
    pub(crate) probe_spec: SurfaceSpec,
    pub(crate) probe_tint: bool,
    pub(crate) probe_magnify: f32,
    pub(crate) probe_disp: f32,
    pub(crate) probe_fill: f32,
}

impl State {
    pub(crate) fn new(cx: &mut Context<Gallery>) -> Self {
        Self {
            probe_knobs: std::array::from_fn(|_| cx.focus_handle()),
            probe_at: Floating::new(Painter::of(cx)),
            // 168 x 168 at r34, as fractions of the slider ranges.
            probe_w: (168.0 - 120.0) / 480.0,
            probe_h: (168.0 - 30.0) / 220.0,
            probe_r: 34.0 / 84.0,
            probe_bg: 5,
            probe_style: SurfaceStyle::Glass(Glass::Clear),
            // The knobs open on the shipped look rather than on copies of its
            // numbers: a theme change moves the probe with it.
            probe_spec: Theme::of(cx).glass_clear,
            probe_tint: false,
            probe_magnify: (Theme::of(cx).glass_magnify + 16.0) / 32.0,
            probe_disp: Theme::of(cx).glass_dispersion,
            probe_fill: 0.34,
        }
    }
}
