//! Inline text: flattening to runs, painting, carets and selection rects.

use super::*;

/// Inline content flattened for shaping: one string, its runs, and the ranges
/// that need painting underneath (link clicks, inline-code washes, mentions).
///
/// [`Self::text`] is the [`Text`] with every mention replaced by what it shows,
/// so every range here is in its offsets; [`Self::shown`] maps the two.
pub struct Flat {
    pub text: SharedString,
    pub runs: Vec<TextRun>,
    pub links: Vec<(Range<usize>, String)>,
    pub code: Vec<Range<usize>>,
    pub mentions: Vec<Mention>,
    pub shown: Shown,
}

/// A mention as it is shown: a favicon slot, then its label and title.
#[derive(Clone, Debug)]
pub struct Mention {
    pub url: String,
    /// Everything it shows, the favicon slot included.
    pub range: Range<usize>,
    /// The favicon slot: one em space.
    pub icon: Range<usize>,
    pub favicon: Option<SharedString>,
    /// What stands in the slot while no favicon has loaded.
    pub initial: SharedString,
}

/// Where a mention's range in a [`Text`] and its range in a [`Flat`] line up,
/// in document order, as `(text, flat)` pairs.
///
/// An offset inside a mention's text has no place in what it shows, and one
/// inside what it shows has none in the text: each lands on an end.
#[derive(Clone, Debug, Default)]
pub struct Shown(Rc<[(Range<usize>, Range<usize>)]>);

impl Shown {
    /// Where a text offset shows. One inside a mention lands on its start.
    pub fn at(&self, offset: usize) -> usize {
        self.map(offset, false)
    }

    /// Where a text range shows. An end inside a mention takes all of it.
    pub fn range(&self, range: &Range<usize>) -> Range<usize> {
        self.map(range.start, false)..self.map(range.end, true)
    }

    fn map(&self, offset: usize, end: bool) -> usize {
        let mut shift = 0isize;
        for (text, flat) in self.0.iter() {
            if offset <= text.start {
                break;
            }
            if offset < text.end {
                return if end { flat.end } else { flat.start };
            }
            shift = flat.end as isize - text.end as isize;
        }
        offset.saturating_add_signed(shift)
    }

    /// The text offset of a shown one. One inside a mention lands on the
    /// nearer end.
    pub fn offset(&self, shown: usize) -> usize {
        let mut shift = 0isize;
        for (text, flat) in self.0.iter() {
            if shown <= flat.start {
                break;
            }
            if shown < flat.end {
                return if shown - flat.start <= flat.end - shown {
                    text.start
                } else {
                    text.end
                };
            }
            shift = text.end as isize - flat.end as isize;
        }
        shown.saturating_add_signed(shift)
    }
}

/// What stands in a mention's favicon slot. An em space is as wide as the
/// type is tall.
const ICON_SLOT: &str = "\u{2003}";

/// Marks are ranges, gpui wants consecutive runs — so cut the text at every
/// mark boundary and ask which marks cover each piece.
pub fn flatten(text: &Text, base_weight: FontWeight, theme: &Theme) -> Flat {
    flatten_with(text, base_weight, theme, |_| None, |_| None)
}

/// [`flatten`] with the app's own marks painted — see [`crate::MarkPaint`] —
/// and each mention described by `preview`. A name the app does not paint
/// reads as the text it wraps.
pub fn flatten_with(
    text: &Text,
    base_weight: FontWeight,
    theme: &Theme,
    paint: impl Fn(&str) -> Option<crate::MarkPaint>,
    preview: impl Fn(&str) -> Option<preview::Preview>,
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

    // Mentions in order; one overlapping an earlier one is text.
    let mut atoms: Vec<(Range<usize>, &str)> = text
        .marks
        .iter()
        .filter_map(|span| match &span.mark {
            Mark::Mention { url, .. } if !span.range.is_empty() => {
                Some((span.range.clone(), url.as_str()))
            }
            _ => None,
        })
        .collect();
    atoms.sort_by_key(|(range, _)| range.start);
    atoms.dedup_by(|later, earlier| later.0.start < earlier.0.end);

    let mut shown = String::with_capacity(text.text.len());
    let mut runs = Vec::new();
    let mut links: Vec<(Range<usize>, String)> = Vec::new();
    let mut code: Vec<Range<usize>> = Vec::new();
    let mut mentions: Vec<Mention> = Vec::new();
    let mut map: Vec<(Range<usize>, Range<usize>)> = Vec::new();

    for pair in cuts.windows(2) {
        let (start, end) = (pair[0], pair[1]);
        let atom = atoms
            .iter()
            .find(|(range, _)| range.start <= start && end <= range.end);
        if atom.is_some_and(|(range, _)| range.start != start) {
            continue;
        }
        let covering = text
            .marks
            .iter()
            .filter(|span| span.range.start <= start && span.range.end >= end);

        let (mut bold, mut italic, mut mono, mut strike) = (false, false, false, false);
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
                Mark::Mention { .. } => {}
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

        let mut face = font(if mono && atom.is_none() {
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
        let underline = UnderlineStyle {
            color: Some(theme.text_muted),
            thickness: px(1.0),
            wavy: false,
        };

        if let Some((range, url)) = atom {
            let described = preview(url).unwrap_or_default();
            let host = preview::host(url);
            let own = &text.text[range.clone()];
            let title: SharedString = match &described.title {
                Some(title) => title.clone(),
                None if own != *url => own.to_string().into(),
                None => host.to_string().into(),
            };
            let from = shown.len();
            let mut run = |shown: &mut String,
                           piece: &str,
                           color: Hsla,
                           underline: Option<UnderlineStyle>| {
                shown.push_str(piece);
                runs.push(TextRun {
                    len: piece.len(),
                    font: face.clone(),
                    color,
                    background_color: None,
                    underline,
                    strikethrough: None,
                });
            };
            run(&mut shown, ICON_SLOT, gpui::transparent_black(), None);
            run(&mut shown, " ", theme.text, None);
            if let Some(label) = described.label.filter(|_| described.title.is_some()) {
                run(&mut shown, &label, theme.text_muted, None);
                run(&mut shown, " ", theme.text, None);
            }
            run(&mut shown, &title, theme.text, Some(underline));
            let to = shown.len();
            links.push((from..to, url.to_string()));
            mentions.push(Mention {
                url: url.to_string(),
                range: from..to,
                icon: from..from + ICON_SLOT.len(),
                favicon: described.icon,
                initial: host
                    .chars()
                    .next()
                    .unwrap_or('?')
                    .to_uppercase()
                    .to_string()
                    .into(),
            });
            map.push((range.clone(), from..to));
            continue;
        }

        let (from, to) = (shown.len(), shown.len() + end - start);
        shown.push_str(&text.text[start..end]);
        if mono {
            match code.last_mut() {
                Some(range) if range.end == from => range.end = to,
                _ => code.push(from..to),
            }
        }
        if let Some(url) = &link {
            match links.last_mut() {
                Some((range, last)) if range.end == from && last == url => range.end = to,
                _ => links.push((from..to, url.clone())),
            }
        }

        runs.push(TextRun {
            len: end - start,
            font: face,
            // Links stay monochrome and underlined; the accent is reserved for
            // primary actions.
            color: match (mono, custom.color) {
                (_, Some(color)) => color,
                (true, None) => theme.code_text,
                (false, None) => theme.text,
            },
            background_color: custom.background,
            underline: (link.is_some() || custom.underline).then_some(underline),
            strikethrough: strike.then_some(StrikethroughStyle {
                thickness: px(1.0),
                color: Some(theme.text_muted),
            }),
        });
    }

    Flat {
        text: shown.into(),
        runs,
        links,
        code,
        mentions,
        shown: Shown(map.into()),
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
    let flat = flatten_with(
        text,
        weight,
        theme,
        |name| crate::marks::paint_of(cx, name, theme),
        |url| preview::of(cx, url),
    );
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
    let shown = flat.shown.clone();
    let caret = overlay.caret_painted().map(|offset| shown.at(offset));
    let selected = overlay.selected(len).map(|range| shown.range(&range));
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

    let mentions = flat.mentions;
    let painted: AnyElement =
        if flat.links.is_empty() {
            styled.into_any_element()
        } else {
            let (ranges, urls): (Vec<_>, Vec<_>) = flat.links.into_iter().unzip();
            let hovered: Vec<(Range<usize>, String)> = mentions
                .iter()
                .map(|mention| (mention.range.clone(), mention.url.clone()))
                .collect();
            let text = InteractiveText::new(ElementId::named_usize("md-text", ix), styled)
                .on_click(ranges, move |clicked, window, cx| {
                    if let Some(url) = urls.get(clicked) {
                        crate::link::open(url, window, cx);
                    }
                });
            match hovered.is_empty() {
                true => text.into_any_element(),
                false => text
                    .tooltip(move |at, _window, cx| {
                        let (_, url) = hovered.iter().find(|(range, _)| range.contains(&at))?;
                        Some(MentionCard::view(url, cx))
                    })
                    .into_any_element(),
            }
        };

    // The wash is painted before the text — an earlier sibling is underneath —
    // reading glyph geometry from the text's own layout handle. Pure paint,
    // never part of layout.
    let wash = theme.code_wash;
    let code_ranges = flat.code;
    let (icon_color, icon_wash) = (theme.text_muted, theme.element_hover);
    let caret_color = theme.caret;
    let selection_color = theme.selection;
    let annotated: Vec<_> = overlay
        .annotated(len, theme)
        .into_iter()
        .map(|(range, wash)| (shown.range(&range), wash))
        .collect();
    let layouts = overlay.layouts.cloned();
    let underlay = canvas(
        {
            let mentions = mentions.clone();
            move |_, window, cx| {
                mentions
                    .iter()
                    .map(|mention| {
                        let favicon = mention.favicon.clone()?;
                        let source = gpui::Resource::Uri(favicon.to_string().into());
                        window
                            .use_asset::<gpui::ImgResourceLoader>(&source, cx)?
                            .ok()
                    })
                    .collect::<Vec<_>>()
            }
        },
        move |_, favicons, window, cx| {
            if let Some(layouts) = &layouts {
                layouts.record(ix, part, span.clone(), layout.clone(), shown.clone());
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
                let shape = ui::input::caret_shape(cx);
                window.paint_quad(
                    shape.quad(
                        caret_quad(head, size, layout.line_height()),
                        (shape != ui::input::CaretShape::Bar)
                            .then(|| caret_advance(&layout, offset))
                            .flatten(),
                        caret_color,
                    ),
                );
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
            for (mention, favicon) in mentions.iter().zip(favicons) {
                let Some(slot) = range_rects(&layout, &mention.icon, 0.0, 0.0).pop() else {
                    continue;
                };
                let side = slot.size.width.min(px(size));
                let icon = Bounds::new(
                    slot.origin + point(px(0.0), (slot.size.height - side) / 2.0),
                    gpui::size(side, side),
                );
                let radius = gpui::Corners::all(side / 4.0);
                if let Some(favicon) = favicon {
                    window
                        .paint_image(icon, icon, radius, favicon, 0, false)
                        .ok();
                    continue;
                }
                window.paint_quad(quad(
                    icon,
                    side / 4.0,
                    icon_wash,
                    px(0.0),
                    gpui::transparent_black(),
                    BorderStyle::default(),
                ));
                let font = font(Theme::of(cx).font_body.clone());
                let letter = window.text_system().shape_line(
                    mention.initial.clone(),
                    side * 0.55,
                    &[TextRun {
                        len: mention.initial.len(),
                        font,
                        color: icon_color,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    }],
                    None,
                );
                let at = point(icon.center().x - letter.width / 2.0, icon.origin.y);
                letter
                    .paint(at, side, gpui::TextAlign::Left, None, window, cx)
                    .ok();
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

/// Resolve the advance in the same shaped line that owns the caret position.
pub(super) fn caret_advance(layout: &TextLayout, offset: usize) -> Option<Pixels> {
    let text = layout.text();
    let mut start = 0;
    for line in layout.line_layouts() {
        let end = start + line.len();
        if offset <= end {
            return ui::input::caret::character_advance(&line, &text[start..end], offset - start);
        }
        start = end + 1;
    }
    None
}
