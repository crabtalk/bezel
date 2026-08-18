use bezel_theme::*;
use gpui::hsla;

fn srgb_u8(c: [f32; 3]) -> [u8; 3] {
    [
        (c[0] * 255.0).round() as u8,
        (c[1] * 255.0).round() as u8,
        (c[2] * 255.0).round() as u8,
    ]
}

#[test]
fn neutral_950_is_0a0a0a() {
    // oklch(0.145 0 0) is Tailwind neutral-950, the reference app background.
    let rgb = srgb_u8(oklch_to_srgb(0.145, 0.0, 0.0));
    assert_eq!(rgb, [10, 10, 10]);
}

#[test]
fn oklch_accents_match_reference() {
    // Reference values computed independently (CSS Color 4 matrices).
    assert_eq!(
        srgb_u8(oklch_to_srgb(0.673, 0.182, 276.935)),
        [124, 134, 255]
    ); // indigo-400
    assert_eq!(
        srgb_u8(oklch_to_srgb(0.704, 0.191, 22.216)),
        [255, 100, 103]
    ); // red-400
    assert_eq!(srgb_u8(oklch_to_srgb(0.828, 0.189, 84.429)), [255, 185, 0]); // amber-400
}

#[test]
fn hsl_roundtrips_through_rgb() {
    for c in [
        Theme::dark().accent,
        Theme::dark().warning,
        Theme::light().accent,
        Theme::light().danger,
        neutral(0.556),
    ] {
        let [r, g, b] = hsl_to_rgb(c.h, c.s, c.l);
        let (h, s, l) = rgb_to_hsl(r, g, b);
        assert!((l - c.l).abs() < 1e-3, "lightness drift for {c:?}");
        assert!((s - c.s).abs() < 1e-3, "saturation drift for {c:?}");
        if c.s > 1e-3 {
            assert!((h - c.h).abs() < 1e-3, "hue drift for {c:?}");
        }
    }
}

#[test]
fn contrast_ratio_hits_known_anchors() {
    let white = grey(0xff);
    let black = grey(0x00);
    assert!((contrast_ratio(white, black) - 21.0).abs() < 0.01);
    assert!((contrast_ratio(white, white) - 1.0).abs() < 0.01);
    // Symmetric regardless of argument order.
    assert!((contrast_ratio(black, white) - contrast_ratio(white, black)).abs() < 1e-4);
}

/// The core claim of the light palette: it is *paired* to dark by contrast
/// ratio, not mirrored by lightness. Each text token must land within 1.0 of
/// its counterpart's ratio against its own background.
#[test]
fn text_contrast_is_paired_across_appearances() {
    let (d, l) = (Theme::dark(), Theme::light());
    for (name, dark_fg, light_fg) in [
        ("text", d.text, l.text),
        ("text_muted", d.text_muted, l.text_muted),
        ("text_faint", d.text_faint, l.text_faint),
    ] {
        let dr = contrast_ratio(dark_fg, d.bg);
        let lr = contrast_ratio(light_fg, l.bg);
        assert!(
            (dr - lr).abs() < 1.0,
            "{name}: dark {dr:.2}:1 vs light {lr:.2}:1 — not a matched pair"
        );
    }
}

/// Body and secondary text must clear WCAG AA (4.5:1) against **both** planes
/// they can land on, in both appearances.
///
/// `text_faint` is held to a lower floor on purpose. It is placeholder and
/// disabled-control copy only, which WCAG 1.4.3 exempts, and the *existing
/// dark palette* already measures ~4.2:1 there (neutral-500 on #060606). The
/// light tone is matched to that inherited number rather than raised past it,
/// so the two appearances stay siblings; raising the floor is a palette
/// decision for both modes at once, not something light mode should do alone.
#[test]
fn text_tones_clear_wcag_aa() {
    for t in [Theme::dark(), Theme::light()] {
        for (name, fg, floor) in [
            ("text", t.text, 4.5),
            ("text_muted", t.text_muted, 4.5),
            ("text_dim", t.text_dim, 4.5),
            ("text_faint", t.text_faint, 4.1),
        ] {
            let on_bg = contrast_ratio(fg, t.bg);
            let on_surface = contrast_ratio(fg, t.surface);
            assert!(
                on_bg >= floor,
                "{:?} {name} on bg is {on_bg:.2}:1, below {floor}",
                t.appearance
            );
            assert!(
                on_surface >= floor,
                "{:?} {name} on surface is {on_surface:.2}:1, below {floor}",
                t.appearance
            );
        }
    }
}

/// Accents are the tokens a naive invert gets most wrong: the dark theme's
/// 400-step indigo/red land near 3:1 on white. The light palette drops to the
/// 600 step at the same hue, which must clear AA for non-text UI (3:1) and,
/// for the accent proper, body-text AA.
#[test]
fn accents_clear_contrast_on_their_background() {
    let l = Theme::light();
    assert!(
        contrast_ratio(l.accent, l.bg) >= 4.5,
        "light accent {:.2}:1",
        contrast_ratio(l.accent, l.bg)
    );
    assert!(
        contrast_ratio(l.danger, l.bg) >= 4.0,
        "light danger {:.2}:1",
        contrast_ratio(l.danger, l.bg)
    );
    for c in [l.warning, l.success, l.busy] {
        assert!(
            contrast_ratio(c, l.bg) >= 3.0,
            "light status color {:.2}:1 — below the 3:1 non-text floor",
            contrast_ratio(c, l.bg)
        );
    }
    // And the dark 400-step accents would NOT have cleared it — this is why
    // the light theme reassigns rather than reuses.
    let d = Theme::dark();
    assert!(
        contrast_ratio(d.warning, l.bg) < 3.0,
        "dark amber-400 unexpectedly passes on white; the invert-is-wrong \
             premise needs rechecking"
    );
}

/// Code is *text*, so syntax tones are held to the body-copy bar, not the
/// 3:1 non-text floor. These are the tokens most likely to be picked by eye
/// from a dark-theme screenshot and silently fail once the page turns white.
#[test]
fn code_and_syntax_tones_are_readable() {
    for t in [Theme::dark(), Theme::light()] {
        for (name, fg) in [
            ("code_text", t.code_text),
            ("syntax_keyword", t.syntax.keyword),
            ("syntax_string", t.syntax.string),
            ("syntax_number", t.syntax.number),
        ] {
            let r = contrast_ratio(fg, t.bg);
            assert!(r >= 4.5, "{:?} {name} is {r:.2}:1 on bg", t.appearance);
        }
        // Diff tints mark whole rows; the 3:1 non-text floor applies.
        for (name, fg) in [("diff_add", t.diff_add), ("diff_del", t.diff_del)] {
            let r = contrast_ratio(fg, t.bg);
            assert!(r >= 3.0, "{:?} {name} is {r:.2}:1 on bg", t.appearance);
        }
    }
}

#[test]
fn syntax_palette_is_readable_on_code_and_diff_backgrounds() {
    for theme in [Theme::dark(), Theme::light()] {
        let add_bg = flatten(theme.diff_add.opacity(0.055), theme.bg);
        let del_bg = flatten(theme.diff_del.opacity(0.055), theme.bg);
        let s = &theme.syntax;
        // Secondary tokens (comments, operators, punctuation) get the 3:1
        // non-text floor; everything else must hold 4.5:1.
        let tokens = [
            ("comment", s.comment, 3.0),
            ("keyword", s.keyword, 4.5),
            ("string", s.string, 4.5),
            ("string_special", s.string_special, 4.5),
            ("escape", s.escape, 4.5),
            ("number", s.number, 4.5),
            ("boolean", s.boolean, 4.5),
            ("type_name", s.type_name, 4.5),
            ("type_builtin", s.type_builtin, 4.5),
            ("constructor", s.constructor, 4.5),
            ("function", s.function, 4.5),
            ("function_builtin", s.function_builtin, 4.5),
            ("macro_name", s.macro_name, 4.5),
            ("property", s.property, 4.5),
            ("constant", s.constant, 4.5),
            ("variable", s.variable, 4.5),
            ("variable_special", s.variable_special, 4.5),
            ("parameter", s.parameter, 4.5),
            ("operator", s.operator, 3.0),
            ("punctuation", s.punctuation, 3.0),
            ("tag", s.tag, 4.5),
            ("attribute", s.attribute, 4.5),
            ("label", s.label, 4.5),
            ("invalid", s.invalid, 4.5),
        ];
        for (token, color, floor) in tokens {
            for (name, background) in [("code", theme.bg), ("add", add_bg), ("del", del_bg)] {
                let ratio = contrast_ratio(color, background);
                assert!(
                    ratio >= floor,
                    "{:?} {token} is {ratio:.2}:1 on {name}",
                    theme.appearance
                );
            }
        }
    }
}

/// The caret is a 2px bar, so the 3:1 non-text floor applies — but it is the
/// one element the user is actively hunting for, and the dark-mode blue is
/// far too light to survive on white unchanged.
#[test]
fn caret_is_findable_on_its_background() {
    for t in [Theme::dark(), Theme::light()] {
        let r = contrast_ratio(t.caret, t.bg);
        assert!(r >= 3.0, "{:?} caret is {r:.2}:1 on bg", t.appearance);
    }
}

/// Solid (primary button) plates must carry their label at AA in both modes.
///
/// The accent plate is held to 4.0 rather than 4.5: dark mode's indigo-500
/// fill — inherited unchanged from the original palette — measures 4.38:1
/// under white, which clears WCAG AA for the medium-weight 14px labels these
/// buttons use (large-text AA is 3:1) but not body copy. Light mode's
/// indigo-600 clears the stricter bar with room to spare.
#[test]
fn solid_button_is_legible_in_both_appearances() {
    for t in [Theme::dark(), Theme::light()] {
        let r = contrast_ratio(t.on_solid, t.solid);
        assert!(r >= 7.0, "{:?} solid button {r:.2}:1", t.appearance);
        let a = contrast_ratio(t.on_accent, t.accent_strong);
        assert!(a >= 4.0, "{:?} accent button {a:.2}:1", t.appearance);
    }
}

/// Surfaces must stay *distinguishable*, but the direction differs: dark
/// stacks upward in lightness, light puts the content plane on top and lets
/// chrome recede. Asserting separation (not a fixed order) is the point.
#[test]
fn surfaces_are_separated_in_both_appearances() {
    let d = Theme::dark();
    assert!(d.bg.l < d.surface.l, "dark: chrome sits above content");
    assert!(d.surface.l < d.surface_raised.l, "dark: raised is lighter");

    let l = Theme::light();
    assert!(
        l.surface.l < l.bg.l,
        "light: chrome recedes *below* the content plane"
    );
    assert!(
        (l.bg.l - l.surface.l) > 0.015,
        "light: sidebar must be visibly separated from the panel"
    );
    // Raised surfaces are white in light mode; separation comes from the
    // border, so the border must be strong enough to carry it alone.
    assert!(contrast_ratio(flatten(l.border, l.bg), l.bg) > 1.15);
}

/// The dark elevation steps are small but deliberate, and each plane must
/// stay strictly above the one below. This test exists because collapsing the
/// ladder onto a single `surface_raised` is the tempting simplification — and
/// it visibly lifts every popover off its plane.
#[test]
fn dark_elevation_ladder_is_strictly_ordered() {
    let d = Theme::dark();
    let ladder = [
        ("bg", d.bg),
        ("surface_card", d.surface_card),
        ("surface_dialog", d.surface_dialog),
        ("surface_overlay", d.surface_overlay),
        ("surface_raised", d.surface_raised),
    ];
    for pair in ladder.windows(2) {
        let ((lower, lo), (upper, hi)) = (pair[0], pair[1]);
        assert!(
            lo.l < hi.l,
            "dark: {upper} ({:.4}) must sit above {lower} ({:.4})",
            hi.l,
            lo.l
        );
    }
}

/// Light mode flattens the ladder onto white on purpose — separation comes
/// from border and shadow. Assert that explicitly so nobody "fixes" it by
/// reintroducing lightness steps that would tint popovers grey.
#[test]
fn light_elevation_is_flat_white_and_leans_on_borders() {
    let l = Theme::light();
    for (name, c) in [
        ("surface_card", l.surface_card),
        ("surface_dialog", l.surface_dialog),
        ("surface_overlay", l.surface_overlay),
    ] {
        assert_eq!(c.l, 1.0, "light {name} should be white");
    }
    // With no lightness step available, the border is the only separator —
    // it has to actually register against the plane behind it.
    assert!(contrast_ratio(flatten(l.border, l.bg), l.bg) > 1.15);
}

/// `surface_raised` is the *bare plate* tone — user message bubbles, the
/// jump-to-bottom pill. Unlike the popover ladder it gets no border and no
/// shadow, so lightness is the only thing separating it from the panel. It
/// was white in light mode once, which made the user's own messages
/// indistinguishable from the page.
#[test]
fn bare_plates_are_visible_against_their_panel() {
    for t in [Theme::dark(), Theme::light()] {
        let delta = (t.surface_raised.l - t.bg.l).abs();
        assert!(
            delta > 0.03,
            "{:?} surface_raised ({:.3}) is only {delta:.3} from bg ({:.3}) — \
                 a plate with no border needs lightness to read",
            t.appearance,
            t.surface_raised.l,
            t.bg.l
        );
        // And hovering it has to go somewhere visible too.
        let hover_delta = (t.surface_raised_hover.l - t.surface_raised.l).abs();
        assert!(
            hover_delta > 0.02,
            "{:?} raised-plate hover moves only {hover_delta:.3}",
            t.appearance
        );
    }
}

/// Monochrome discipline: neutrals carry no saturation in either appearance.
#[test]
fn neutrals_are_achromatic() {
    for t in [Theme::dark(), Theme::light()] {
        for c in [
            t.bg,
            t.surface,
            t.surface_raised,
            t.text,
            t.text_muted,
            t.text_faint,
            t.solid,
            t.on_solid,
        ] {
            assert_eq!(c.s, 0.0, "{:?} neutral has chroma", t.appearance);
            assert_eq!(c.a, 1.0, "{:?} neutral is translucent", t.appearance);
        }
    }
}

#[test]
fn hairlines_and_washes_flip_tone_with_appearance() {
    let _guard = lock_appearance();
    set_current_appearance(Appearance::Dark);
    assert_eq!(hairline(0.1).l, 1.0, "dark hairlines are white");
    assert_eq!(ink(0.1).l, 1.0, "dark fills are white");
    assert_eq!(ink(0.1).a, 0.1, "dark alphas pass through untouched");
    assert_eq!(wash(0.14).l, 0.92, "dark washes are soft-white");

    set_current_appearance(Appearance::Light);
    assert_eq!(hairline(0.1).l, 0.0, "light hairlines are black");
    assert_eq!(ink(0.1).l, 0.0, "light fills are black");
    assert_eq!(wash(0.14).l, 0.10, "light washes are soft-black");
    // Fills keep their alpha; only hairlines are scaled.
    assert_eq!(ink(0.10).a, 0.10, "light fills keep their alpha");
    assert!(hairline(0.10).a > 0.10, "light hairlines strengthen");
    assert!(hairline(0.60).a <= 0.5, "hairline alpha is capped");

    set_current_appearance(Appearance::Dark);
}

/// A hover wash has to actually be *visible* against the surface it lands on,
/// in both appearances — the failure mode of a halved light alpha.
#[test]
fn hover_wash_is_visible_on_its_surface() {
    let _guard = lock_appearance();
    for (appearance, theme) in [
        (Appearance::Dark, Theme::dark()),
        (Appearance::Light, Theme::light()),
    ] {
        set_current_appearance(appearance);
        let hovered = flatten(wash(0.14), theme.surface);
        let delta = (hovered.l - theme.surface.l).abs();
        assert!(
            delta > 0.02,
            "{appearance:?} hover wash shifts lightness by only {delta:.4}"
        );
    }
    set_current_appearance(Appearance::Dark);
}

/// The regression that shipped: subtle fills are quoted at very low alphas
/// (`ink(0.03)` is the composer plate, `ink(0.05)` a key cap), and scaling
/// those down for light mode erased them — the composer rendered as bare text
/// on white. Assert the faintest fill we actually use still moves the surface
/// it lands on, in *both* appearances.
#[test]
fn faintest_fills_survive_in_both_appearances() {
    let _guard = lock_appearance();
    for (appearance, theme) in [
        (Appearance::Dark, Theme::dark()),
        (Appearance::Light, Theme::light()),
    ] {
        set_current_appearance(appearance);
        for alpha in [0.03, 0.05] {
            let plate = flatten(ink(alpha), theme.bg);
            let delta = (plate.l - theme.bg.l).abs();
            assert!(
                delta >= 0.02,
                "{appearance:?} ink({alpha}) shifts its background by only \
                     {delta:.4} — the fill is invisible"
            );
        }
    }
    set_current_appearance(Appearance::Dark);
}

/// Both appearances are glass-forward on macOS. Light frost runs heavier
/// than dark's (a light tint controls the blur less), and floating cards
/// step their tint coverage up in light so menu text stays on a
/// known-enough background — assert both relationships so the frost and
/// the overlay can't drift apart.
#[test]
fn both_appearances_stay_frosted_and_light_runs_heavier() {
    if Theme::GLASS_ALPHA < 1.0 {
        let (dark, light) = (Theme::dark(), Theme::light());
        assert!(dark.glass().a < 1.0, "dark keeps its translucent frost");
        assert!(light.glass().a < 1.0, "light is glass-forward like dark");
        assert!(
            light.glass().a > dark.glass().a - f32::EPSILON,
            "a light tint dominates the blur less, so it must not run looser than dark"
        );
        assert!(
            light.glass_overlay().a > dark.glass_overlay().a,
            "light floating cards need more coverage over blur for legible rows"
        );
    } else {
        assert_eq!(Theme::light().glass().a, 1.0);
        assert_eq!(Theme::dark().glass().a, 1.0);
    }
}

/// An input plate has to read as *lifted* in both appearances. Dark does that
/// with a faint white wash; the literal light translation is a faint black
/// wash, which reads as a dent instead — so light lifts with white plus its
/// border. Assert the plate is never darker than the panel it sits on.
#[test]
fn input_plate_never_reads_as_recessed() {
    for t in [Theme::dark(), Theme::light()] {
        let plate = flatten(t.input_bg, t.bg);
        assert!(
            plate.l >= t.bg.l,
            "{:?} input plate ({:.3}) is darker than its panel ({:.3}) — \
                 that reads as recessed, not raised",
            t.appearance,
            plate.l,
            t.bg.l
        );
    }
}

/// Card rows fill with translucent washes, and a drop shadow behind a
/// translucent fill shows through as a grey plate — selection inside a
/// floating card must be edge-only. This regressed once: light menu rows
/// borrowed the glass-chip recipe, drop shadow included.
#[test]
fn card_selection_paints_nothing_behind_its_row() {
    let _guard = lock_appearance();
    for appearance in [Appearance::Dark, Appearance::Light] {
        set_current_appearance(appearance);
        for shadow in card_selected_shadows() {
            assert!(
                shadow.inset,
                "{appearance:?}: card selection may only paint inset edges"
            );
        }
    }
    set_current_appearance(Appearance::Dark);
}

/// Glass selection is edge-only in BOTH appearances — no drop-shadow seat.
/// Every light seat tried (10% tight, 6%+5% pair, lone 4%) read as a grey
/// rim or a coarse smudge, and the tab strip clips escaping shadows
/// vertically (user reports). The ring must also stay subtle enough to
/// define the chip rather than frame it.
#[test]
fn glass_selection_is_edge_only_and_subtle() {
    let _guard = lock_appearance();
    for appearance in [Appearance::Dark, Appearance::Light] {
        set_current_appearance(appearance);
        let shadows = glass_selected_shadows();
        assert!(
            shadows.iter().all(|s| s.inset),
            "{appearance:?}: glass selection may only paint inset edges"
        );
        let ring = shadows.iter().find(|s| s.inset).expect("selection ring");
        assert!(
            ring.color.a <= 0.09,
            "{appearance:?}: ring at {:.2} alpha frames the chip instead of defining it",
            ring.color.a
        );
    }
    set_current_appearance(Appearance::Dark);
}

#[test]
fn appearance_mirror_tracks_installed_theme() {
    let _guard = lock_appearance();
    set_current_appearance(Appearance::Light);
    assert_eq!(current_appearance(), Appearance::Light);
    set_current_appearance(Appearance::Dark);
    assert_eq!(current_appearance(), Appearance::Dark);
}

#[test]
fn window_appearance_maps_onto_ours() {
    use gpui::WindowAppearance as W;
    assert_eq!(Appearance::from_window(W::Light), Appearance::Light);
    assert_eq!(Appearance::from_window(W::VibrantLight), Appearance::Light);
    assert_eq!(Appearance::from_window(W::Dark), Appearance::Dark);
    assert_eq!(Appearance::from_window(W::VibrantDark), Appearance::Dark);
}

#[test]
fn scrim_is_black_but_lighter_in_light_mode() {
    let (d, l) = (Theme::dark(), Theme::light());
    assert_eq!(d.scrim().l, 0.0);
    assert_eq!(l.scrim().l, 0.0);
    assert!(l.scrim().a < d.scrim().a);
}

#[test]
fn mix_endpoints_and_midpoint() {
    let a = hsla(0.0, 0.0, 0.0, 1.0);
    let b = hsla(0.5, 1.0, 1.0, 0.0);
    assert_eq!(mix(a, b, 0.0), a);
    assert_eq!(mix(a, b, 1.0), b);
    let mid = mix(a, b, 0.5);
    assert!((mid.l - 0.5).abs() < 1e-6 && (mid.a - 0.5).abs() < 1e-6);
    // Out-of-range t clamps.
    assert_eq!(mix(a, b, 2.0), b);
}

/// The rule the library now derives nested corners from, rather than the
/// values it happens to produce today: a row inside a card keeps its corners
/// parallel to the card's, and an inset at least as deep as the radius
/// squares them off instead of going negative.
#[test]
fn inset_radius_is_concentric_and_floors_at_zero() {
    // The case the whole pass came out of: `popover_card` is 12 with a 4px
    // inset, and 8.0 was the crate's most-repeated corner value.
    assert_eq!(Theme::inset_radius(Theme::SURFACE_RADIUS, 4.0), 8.0);
    // The segmented track, the other pair that turned out to be derived.
    assert_eq!(Theme::inset_radius(9.0, 2.0), 7.0);
    // A dialog insets by more than it rounds; its children are square.
    assert_eq!(Theme::inset_radius(16.0, 20.0), 0.0);
    assert_eq!(Theme::inset_radius(8.0, 8.0), 0.0);
}

#[test]
fn layout_numbers_match_the_reference() {
    assert_eq!(Theme::HEADER_HEIGHT, 44.0); // h-11
    assert_eq!(Theme::STATUS_STRIP_HEIGHT, 24.0); // h-6
    assert_eq!(Theme::BUBBLE_RADIUS, 16.0);
}
