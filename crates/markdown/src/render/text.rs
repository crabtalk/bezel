//! Inline text: flattening to runs, painting, carets and selection rects.

use super::*;

/// Inline content flattened for shaping: one string, its runs, and the ranges
/// that need painting underneath (link clicks, inline-code washes, chips).
pub struct Flat {
    pub text: SharedString,
    pub runs: Vec<TextRun>,
    pub links: Vec<(Range<usize>, String)>,
    pub code: Vec<Range<usize>>,
    pub chips: Vec<Range<usize>>,
}

/// Marks are ranges, gpui wants consecutive runs — so cut the text at every
/// mark boundary and ask which marks cover each piece.
pub fn flatten(text: &Text, base_weight: FontWeight, theme: &Theme) -> Flat {
    flatten_with(text, base_weight, theme, |_| None)
}

/// [`flatten`] with the app's own marks painted — see [`crate::MarkPaint`]. A
/// name the app does not paint reads as the text it wraps.
pub fn flatten_with(
    text: &Text,
    base_weight: FontWeight,
    theme: &Theme,
    paint: impl Fn(&str) -> Option<crate::MarkPaint>,
) -> Flat {
    let mut cuts: Vec<usize> = text
        .marks
        .iter()
        .flat_map(|span| [span.range.start, span.range.end])
        .chain([0, text.text.len()])
        .filter(|cut| *cut <= text.text.len())
        .collect();
    cuts.sort_unstable();
    cuts.dedup();

    let mut runs = Vec::new();
    let mut links: Vec<(Range<usize>, String)> = Vec::new();
    let mut code: Vec<Range<usize>> = Vec::new();
    let mut chips: Vec<Range<usize>> = Vec::new();

    for pair in cuts.windows(2) {
        let (start, end) = (pair[0], pair[1]);
        let covering = text
            .marks
            .iter()
            .filter(|span| span.range.start <= start && span.range.end >= end);

        let (mut bold, mut italic, mut mono, mut strike) = (false, false, false, false);
        let mut chip = false;
        let mut link = None;
        // The app's own marks, merged in the order they cover this run: the
        // last one to say something about a field is the one that says it.
        let mut custom = crate::MarkPaint::default();
        for span in covering {
            match &span.mark {
                Mark::Bold => bold = true,
                Mark::Italic => italic = true,
                Mark::Strike => strike = true,
                Mark::Code => mono = true,
                Mark::Mention { url, .. } => {
                    chip = true;
                    link = Some(url.clone());
                }
                Mark::Link(url) | Mark::Image(url) => link = Some(url.clone()),
                Mark::Custom(name) => {
                    let Some(painted) = paint(name) else { continue };
                    custom.color = painted.color.or(custom.color);
                    custom.background = painted.background.or(custom.background);
                    custom.weight = painted.weight.or(custom.weight);
                    custom.italic |= painted.italic;
                    custom.underline |= painted.underline;
                    custom.strikethrough |= painted.strikethrough;
                }
            }
        }
        let (italic, strike) = (italic || custom.italic, strike || custom.strikethrough);

        if mono {
            match code.last_mut() {
                Some(range) if range.end == start => range.end = end,
                _ => code.push(start..end),
            }
        }
        if chip {
            match chips.last_mut() {
                Some(range) if range.end == start => range.end = end,
                _ => chips.push(start..end),
            }
        }
        if let Some(url) = &link {
            match links.last_mut() {
                Some((range, last)) if range.end == start && last == url => range.end = end,
                _ => links.push((start..end, url.clone())),
            }
        }

        let mut face = font(if mono {
            theme.font_mono.clone()
        } else {
            theme.font_body.clone()
        });
        face.weight = if bold && base_weight.0 < FontWeight::SEMIBOLD.0 {
            FontWeight::SEMIBOLD
        } else {
            custom.weight.unwrap_or(base_weight)
        };
        face.style = if italic {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        };

        runs.push(TextRun {
            len: end - start,
            font: face,
            // Links stay monochrome and underlined; the accent is reserved for
            // primary actions. A chip carries its own wash, so underlining it
            // too would say the same thing twice.
            color: match (mono, custom.color) {
                (_, Some(color)) => color,
                (true, None) => theme.code_text,
                (false, None) => theme.text,
            },
            background_color: custom.background,
            underline: ((link.is_some() && !chip) || custom.underline).then_some(UnderlineStyle {
                color: Some(theme.text_muted),
                thickness: px(1.0),
                wavy: false,
            }),
            strikethrough: strike.then_some(StrikethroughStyle {
                thickness: px(1.0),
                color: Some(theme.text_muted),
            }),
        });
    }

    Flat {
        text: text.text.clone().into(),
        runs,
        links,
        code,
        chips,
    }
}

pub(super) fn text_element(
    text: &Text,
    size: f32,
    line_height: f32,
    weight: FontWeight,
    overlay: Overlay,
    theme: &Theme,
    cx: &App,
) -> AnyElement {
    let flat = flatten_with(text, weight, theme, |name| {
        crate::marks::paint_of(cx, name, theme)
    });
    painted_text(flat, text.text.len(), size, line_height, overlay, theme)
}

/// Shaped inline content with the editing overlay under it: the selection, the
/// caret, the inline-code wash, and the layout a click resolves against.
///
/// Takes a [`Flat`] rather than a [`Text`] because a table has to shape every
/// cell to measure the columns before it can paint one.
pub(super) fn painted_text(
    flat: Flat,
    len: usize,
    size: f32,
    line_height: f32,
    overlay: Overlay,
    theme: &Theme,
) -> AnyElement {
    let (ix, part) = (overlay.block, overlay.part);
    let (caret, selected) = (overlay.caret_painted(), overlay.selected(len));
    let span = 0..len;
    // Only where the caret already is, and only while there is nothing to
    // read: a hint on every empty block would be a page of grey.
    let hint = overlay
        .placeholder
        // The caret's own presence, not the blink's phase — a hint that came
        // and went twice a second would be unreadable.
        .filter(|_| len == 0 && overlay.caret().is_some())
        .map(|hint| {
            div()
                .absolute()
                .text_color(theme.text_faint)
                .child(hint.clone())
        });
    let styled = StyledText::new(flat.text).with_runs(flat.runs);
    let layout = styled.layout().clone();

    let painted: AnyElement = if flat.links.is_empty() {
        styled.into_any_element()
    } else {
        let (ranges, urls): (Vec<_>, Vec<_>) = flat.links.into_iter().unzip();
        InteractiveText::new(ElementId::named_usize("md-text", ix), styled)
            .on_click(ranges, move |clicked, _window, cx| {
                if let Some(url) = urls.get(clicked) {
                    cx.open_url(url);
                }
            })
            .into_any_element()
    };

    // The wash is painted before the text — an earlier sibling is underneath —
    // reading glyph geometry from the text's own layout handle. Pure paint,
    // never part of layout.
    let wash = theme.code_wash;
    let code_ranges = flat.code;
    let chip_wash = theme.element_hover;
    let chip_edge = theme.border;
    let chip_ranges = flat.chips;
    let caret_color = theme.caret;
    let selection_color = theme.selection;
    let annotated = overlay.annotated(len, theme);
    let layouts = overlay.layouts.cloned();
    let underlay = canvas(
        |_, _, _| (),
        move |_, _, window, _| {
            if let Some(layouts) = &layouts {
                layouts.record(ix, part, span.clone(), layout.clone());
            }
            // Below the selection, so dragging across a comment still reads as
            // selected rather than as a third colour nobody chose.
            for (range, wash) in &annotated {
                for rect in range_rects(&layout, range, 0.0, 0.0) {
                    window.paint_quad(quad(
                        rect,
                        px(2.0),
                        *wash,
                        px(0.0),
                        gpui::transparent_black(),
                        BorderStyle::default(),
                    ));
                }
            }
            // Under the glyphs, like the inline-code wash — one quad per visual
            // row, so a wrapped selection is a stack of rows rather than a box
            // around all of them.
            if let Some(range) = &selected {
                for rect in range_rects(&layout, range, 0.0, 0.0) {
                    window.paint_quad(quad(
                        rect,
                        px(2.0),
                        selection_color,
                        px(0.0),
                        gpui::transparent_black(),
                        BorderStyle::default(),
                    ));
                }
            }
            if let Some(offset) = caret
                && let Some(head) = layout.position_for_index(offset)
            {
                window.paint_quad(quad(
                    caret_quad(head, size, layout.line_height()),
                    px(0.0),
                    caret_color,
                    px(0.0),
                    gpui::transparent_black(),
                    BorderStyle::default(),
                ));
            }
            for range in &code_ranges {
                for rect in range_rects(&layout, range, INLINE_CODE_PAD_X, INLINE_CODE_INSET_Y) {
                    window.paint_quad(quad(
                        rect,
                        px(INLINE_CODE_RADIUS),
                        wash,
                        px(0.0),
                        gpui::transparent_black(),
                        BorderStyle::default(),
                    ));
                }
            }
            // Wider, rounder and outlined, so a chip and an inline code span
            // never read as the same thing at a glance.
            for range in &chip_ranges {
                for rect in range_rects(&layout, range, CHIP_PAD_X, CHIP_INSET_Y) {
                    window.paint_quad(quad(
                        rect,
                        px(Theme::control_radius()),
                        chip_wash,
                        px(1.0),
                        chip_edge,
                        BorderStyle::Solid,
                    ));
                }
            }
        },
    )
    .absolute()
    .size_full();

    div()
        .text_size(px(size))
        .line_height(px(line_height))
        .relative()
        .child(underlay)
        .children(hint)
        .child(painted)
        .into_any_element()
}

/// The caret's quad: the text's own size, centred in the line box.
///
/// The leading is not the caret's to take. A document is set with air around
/// its lines, and a caret filling all of it reads as a second, larger font
/// standing where the text should be.
pub(super) fn caret_quad(head: Point<Pixels>, size: f32, line_height: Pixels) -> Bounds<Pixels> {
    let inset = (line_height - px(size)) / 2.0;
    Bounds::new(
        head + point(px(0.0), inset),
        gpui::size(px(CARET_WIDTH), px(size)),
    )
}

/// The rectangles a byte range occupies, one per visual row.
pub(super) fn range_rects(
    layout: &gpui::TextLayout,
    range: &Range<usize>,
    pad_x: f32,
    inset_y: f32,
) -> Vec<Bounds<Pixels>> {
    let mut rects = Vec::new();
    let line_height = layout.line_height();
    let mut origin = layout.bounds().origin;
    let mut line_start = 0;
    for line in layout.line_layouts() {
        let shaped = &line.unwrapped_layout;
        // A wrap boundary index is both the end of one row and the start of
        // the next.
        let row_ends = line
            .wrap_boundaries()
            .iter()
            .map(|wrap| shaped.runs[wrap.run_ix].glyphs[wrap.glyph_ix].index)
            .chain([line.len()]);
        let mut row_start = 0;
        for (row, row_end) in row_ends.enumerate() {
            let from = range
                .start
                .saturating_sub(line_start)
                .clamp(row_start, row_end);
            let to = range.end.saturating_sub(line_start).min(row_end);
            let row_x = shaped.x_for_index(row_start);
            let (left, right) = (shaped.x_for_index(from), shaped.x_for_index(to));
            if from < to && right > left {
                rects.push(Bounds::new(
                    origin
                        + point(
                            left - row_x - px(pad_x),
                            line_height * row as f32 + px(inset_y),
                        ),
                    size(
                        right - left + px(2.0 * pad_x),
                        line_height - px(2.0 * inset_y),
                    ),
                ));
            }
            row_start = row_end;
        }
        origin.y += line.size(line_height).height;
        // The newline between two lines is a byte of the text and of neither.
        line_start += line.len() + 1;
    }
    rects
}
