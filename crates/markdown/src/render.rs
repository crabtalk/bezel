//! [`Doc`] → gpui elements.
//!
//! Numbers drive layout (sizes, line heights, paddings — the constants here);
//! colors are paint, read from [`Theme`]. Blocks are a flat list, so nesting is
//! left padding rather than nested containers, and the gap between two blocks
//! is decided by the pair: list items sit tight, everything else breathes.
//!
//! Ported from zeronsh/comet (MIT) and rebuilt against the flat block model.

use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use bezel_theme::Theme;
use gpui::{
    AnyElement, App, BorderStyle, Bounds, ElementId, FontStyle, FontWeight, InteractiveText,
    Pixels, Point, SharedString, StrikethroughStyle, StyledText, TextLayout, TextRun,
    UnderlineStyle, Window, canvas, div, font, img, point, prelude::*, px, quad, size,
};

use crate::doc::{Align, Block, BlockKind, Doc, Mark, Text};
use crate::edit::Cursor;

/// Space between two ordinary blocks, and the tighter space inside a list.
const BLOCK_GAP: f32 = 12.0;
const LIST_GAP: f32 = 4.0;
/// Body scale.
const TEXT_SIZE: f32 = 14.0;
const LINE_HEIGHT: f32 = 22.0;
/// One indent level. Wide enough to clear a marker and read as a level.
const INDENT_WIDTH: f32 = 22.0;
/// The marker column of a list row.
const MARKER_WIDTH: f32 = 18.0;
const MARKER_GAP: f32 = 8.0;
/// Code metrics — a block's height is `lines × CODE_LINE_HEIGHT` plus padding.
const CODE_TEXT_SIZE: f32 = 12.5;
const CODE_LINE_HEIGHT: f32 = 18.0;
const CODE_PADDING_X: f32 = 12.0;
const CODE_PADDING_Y: f32 = 10.0;
/// Inline code's wash is a rounded quad painted under the glyphs: a run's
/// `background_color` can only ever be a square box.
const INLINE_CODE_RADIUS: f32 = 4.5;
const INLINE_CODE_PAD_X: f32 = 2.0;
const INLINE_CODE_INSET_Y: f32 = 2.0;
/// Table metrics. The design is frameless: hairlines between rows are the only
/// chrome — no outer box, no header fill, no rounding.
const TABLE_CELL_PADDING: f32 = 12.0;
const TABLE_DIVIDER: f32 = 1.0;
/// Floor for a column's max-content share, so a short column ("1k") beside a
/// prose column keeps a readable width.
const TABLE_MIN_COLUMN_CONTENT: f32 = 48.0;
/// Narrowest a column wraps down to before the table scrolls instead.
const TABLE_MIN_COLUMN_WIDTH: f32 = 96.0;

/// Where each block's text landed, recorded as it painted.
///
/// A caret has to be placeable by pointer, and only paint knows where a glyph
/// ended up. An editor hands one of these in, the renderer fills it, and the
/// next click resolves against it. Read-only callers pass nothing and pay
/// nothing.
#[derive(Clone, Default)]
pub struct BlockLayouts(Rc<RefCell<Vec<(usize, TextLayout)>>>);

impl BlockLayouts {
    /// The block and byte offset under `point`.
    ///
    /// Falls back to the nearest block vertically, so clicking the margin
    /// beside a line — or below the last one — still lands somewhere useful
    /// rather than doing nothing.
    pub fn hit(&self, point: Point<Pixels>) -> Option<Cursor> {
        let entries = self.0.borrow();
        let offset_in = |layout: &TextLayout| {
            let (Ok(offset) | Err(offset)) = layout.index_for_position(point);
            offset
        };
        if let Some((ix, layout)) = entries
            .iter()
            .find(|(_, layout)| layout.bounds().contains(&point))
        {
            return Some(Cursor::new(*ix, offset_in(layout)));
        }
        let (ix, layout) = entries.iter().min_by_key(|(_, layout)| {
            let bounds = layout.bounds();
            let above = (bounds.origin.y - point.y).abs();
            let below = (bounds.origin.y + bounds.size.height - point.y).abs();
            f32::from(above.min(below)) as i64
        })?;
        Some(Cursor::new(*ix, offset_in(layout)))
    }

    fn record(&self, ix: usize, layout: TextLayout) {
        self.0.borrow_mut().push((ix, layout));
    }

    fn clear(&self) {
        self.0.borrow_mut().clear();
    }
}

/// Parse and render in one step — the common case for read-only content.
pub fn markdown(source: &str, window: &mut Window, cx: &mut App) -> AnyElement {
    render(&crate::parse(source), window, cx)
}

/// Render a document.
pub fn render(doc: &Doc, window: &mut Window, cx: &mut App) -> AnyElement {
    render_with_caret(doc, None, None, window, cx)
}

/// Render a document with a caret in it.
///
/// The caret is a paint-time concern and nothing else: it reads its position
/// off the shaped text's own layout handle, the same way the inline-code wash
/// does, so nothing about layout depends on where it sits. An editor supplies
/// the position and owns the focus and the keys; painting one line is not worth
/// a second renderer.
pub fn render_with_caret(
    doc: &Doc,
    caret: Option<Cursor>,
    layouts: Option<&BlockLayouts>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    // Refilled every frame, in paint order.
    if let Some(layouts) = layouts {
        layouts.clear();
    }
    // Cloned once so the theme is readable while `cx` stays free for the
    // element state the copy button needs.
    let theme = Theme::of(cx).clone();
    let mut column = div().flex().flex_col();

    for (ix, block) in doc.blocks.iter().enumerate() {
        let gap = match doc.blocks.get(ix.wrapping_sub(1)) {
            None => 0.0,
            Some(previous) if tight(previous, block) => LIST_GAP,
            Some(_) => BLOCK_GAP,
        };
        let caret = caret
            .filter(|caret| caret.block == ix)
            .map(|caret| caret.offset);
        column = column.child(
            div()
                .mt(px(gap))
                .pl(px(block.indent as f32 * INDENT_WIDTH))
                .child(block_element(block, ix, caret, layouts, &theme, window, cx)),
        );
    }

    column.into_any_element()
}

/// Whether two adjacent blocks belong to the same list and should sit close.
fn tight(previous: &Block, next: &Block) -> bool {
    let marker = |block: &Block| {
        matches!(
            block.kind,
            BlockKind::Bullet(_) | BlockKind::Ordered { .. } | BlockKind::Task { .. }
        )
    };
    marker(previous) && (marker(next) || next.indent > previous.indent)
}

#[allow(clippy::too_many_arguments)]
fn block_element(
    block: &Block,
    ix: usize,
    caret: Option<usize>,
    layouts: Option<&BlockLayouts>,
    theme: &Theme,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    match &block.kind {
        BlockKind::Paragraph(text) => text_element(
            text,
            TEXT_SIZE,
            LINE_HEIGHT,
            FontWeight::NORMAL,
            ix,
            caret,
            layouts,
            theme,
        ),
        BlockKind::Heading { level, text } => {
            let (size, line) = heading_metrics(*level);
            text_element(
                text,
                size,
                line,
                FontWeight::SEMIBOLD,
                ix,
                caret,
                layouts,
                theme,
            )
        }
        BlockKind::Bullet(text) => marker_row(disc(theme), text, ix, caret, layouts, theme),
        BlockKind::Ordered { number, text } => marker_row(
            div()
                .flex_none()
                .w(px(MARKER_WIDTH))
                .text_size(px(TEXT_SIZE))
                .line_height(px(LINE_HEIGHT))
                .text_color(theme.text_muted)
                .child(SharedString::from(format!("{number}.")))
                .into_any_element(),
            text,
            ix,
            caret,
            layouts,
            theme,
        ),
        BlockKind::Task { checked, text } => {
            marker_row(checkbox(*checked, theme), text, ix, caret, layouts, theme)
        }
        BlockKind::Quote(text) => div()
            .border_l_2()
            .border_color(theme.border_strong)
            .pl(px(12.0))
            .pr(px(10.0))
            .py(px(2.0))
            .text_color(theme.text_muted)
            .child(text_element(
                text,
                TEXT_SIZE,
                LINE_HEIGHT,
                FontWeight::NORMAL,
                ix,
                caret,
                layouts,
                theme,
            ))
            .into_any_element(),
        BlockKind::Code { language, code } => {
            code_block(language.as_deref(), code, ix, theme, window, cx)
        }
        BlockKind::Image { url, alt } => image(url, alt, theme),
        BlockKind::Table {
            align,
            header,
            rows,
        } => table(align, header, rows, ix, theme, window),
        BlockKind::Rule => div()
            .h(px(1.0))
            .w_full()
            .bg(theme.border)
            .into_any_element(),
    }
}

/// A tight scale: headings step down toward body size quickly.
fn heading_metrics(level: u8) -> (f32, f32) {
    match level {
        1 => (19.0, 27.0),
        2 => (16.0, 24.0),
        3 => (15.0, 22.0),
        _ => (14.0, 22.0),
    }
}

/// A real 5px disc rather than the "•" glyph, which reads too small at 14px.
fn disc(theme: &Theme) -> AnyElement {
    div()
        .flex_none()
        .w(px(MARKER_WIDTH))
        .h(px(LINE_HEIGHT))
        .flex()
        .items_center()
        .child(
            div()
                .ml(px(1.0))
                .w(px(5.0))
                .h(px(5.0))
                .rounded_full()
                .bg(theme.text_faint),
        )
        .into_any_element()
}

fn checkbox(checked: bool, theme: &Theme) -> AnyElement {
    let mut box_ = div()
        .w(px(13.0))
        .h(px(13.0))
        .rounded(px(3.5))
        .border_1()
        .flex()
        .items_center()
        .justify_center();
    box_ = if checked {
        box_.bg(theme.solid)
            .border_color(theme.solid)
            .text_size(px(9.0))
            .text_color(theme.on_solid)
            .child("✓")
    } else {
        box_.border_color(theme.border_strong)
    };

    div()
        .flex_none()
        .w(px(MARKER_WIDTH))
        .h(px(LINE_HEIGHT))
        .flex()
        .items_center()
        .child(box_)
        .into_any_element()
}

fn marker_row(
    marker: AnyElement,
    text: &Text,
    ix: usize,
    caret: Option<usize>,
    layouts: Option<&BlockLayouts>,
    theme: &Theme,
) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .gap(px(MARKER_GAP))
        .child(marker)
        .child(div().flex_1().min_w_0().child(text_element(
            text,
            TEXT_SIZE,
            LINE_HEIGHT,
            FontWeight::NORMAL,
            ix,
            caret,
            layouts,
            theme,
        )))
        .into_any_element()
}

/// Inline content flattened for shaping: one string, its runs, and the ranges
/// that need painting underneath (link clicks, inline-code washes).
struct Flat {
    text: SharedString,
    runs: Vec<TextRun>,
    links: Vec<(Range<usize>, String)>,
    code: Vec<Range<usize>>,
}

/// Marks are ranges, gpui wants consecutive runs — so cut the text at every
/// mark boundary and ask which marks cover each piece.
fn flatten(text: &Text, base_weight: FontWeight, theme: &Theme) -> Flat {
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

    for pair in cuts.windows(2) {
        let (start, end) = (pair[0], pair[1]);
        let covering = text
            .marks
            .iter()
            .filter(|span| span.range.start <= start && span.range.end >= end);

        let (mut bold, mut italic, mut mono, mut strike) = (false, false, false, false);
        let mut link = None;
        for span in covering {
            match &span.mark {
                Mark::Bold => bold = true,
                Mark::Italic => italic = true,
                Mark::Strike => strike = true,
                Mark::Code => mono = true,
                Mark::Link(url) | Mark::Image(url) => link = Some(url.clone()),
            }
        }

        if mono {
            match code.last_mut() {
                Some(range) if range.end == start => range.end = end,
                _ => code.push(start..end),
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
            theme.font_sans.clone()
        });
        face.weight = if bold && base_weight.0 < FontWeight::SEMIBOLD.0 {
            FontWeight::SEMIBOLD
        } else {
            base_weight
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
            // primary actions.
            color: if mono { theme.code_text } else { theme.text },
            background_color: None,
            underline: link.is_some().then_some(UnderlineStyle {
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
    }
}

#[allow(clippy::too_many_arguments)]
fn text_element(
    text: &Text,
    size: f32,
    line_height: f32,
    weight: FontWeight,
    ix: usize,
    caret: Option<usize>,
    layouts: Option<&BlockLayouts>,
    theme: &Theme,
) -> AnyElement {
    let flat = flatten(text, weight, theme);
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
    let caret_color = theme.caret;
    let layouts = layouts.cloned();
    let underlay = canvas(
        |_, _, _| (),
        move |_, _, window, _| {
            if let Some(layouts) = &layouts {
                layouts.record(ix, layout.clone());
            }
            if let Some(offset) = caret
                && let Some(head) = layout.position_for_index(offset)
            {
                window.paint_quad(quad(
                    Bounds::new(head, gpui::size(px(1.5), layout.line_height())),
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
        },
    )
    .absolute()
    .size_full();

    div()
        .text_size(px(size))
        .line_height(px(line_height))
        .relative()
        .child(underlay)
        .child(painted)
        .into_any_element()
}

/// The rectangles a byte range occupies, one per visual row.
fn range_rects(
    layout: &gpui::TextLayout,
    range: &Range<usize>,
    pad_x: f32,
    inset_y: f32,
) -> Vec<Bounds<Pixels>> {
    let mut rects = Vec::new();
    let line_height = layout.line_height();
    let mut cursor = range.start;
    // Walk one visual row at a time. A wrapped range has no direct row query,
    // so the last index still on this row is found by bisection.
    let mut guard = 0;
    while cursor < range.end && guard < 256 {
        guard += 1;
        let Some(head) = layout.position_for_index(cursor) else {
            break;
        };
        let (row_end, next) = match layout.position_for_index(range.end) {
            Some(tail) if tail.y == head.y => (range.end, range.end),
            _ => {
                let (mut low, mut high) = (cursor, range.end);
                while high - low > 1 {
                    let mid = low + (high - low) / 2;
                    match layout.position_for_index(mid) {
                        Some(probe) if probe.y == head.y => low = mid,
                        _ => high = mid,
                    }
                }
                (low, high)
            }
        };
        if let Some(tail) = layout.position_for_index(row_end)
            && tail.x > head.x
        {
            rects.push(Bounds::new(
                point(head.x - px(pad_x), head.y + px(inset_y)),
                size(
                    tail.x - head.x + px(2.0 * pad_x),
                    line_height - px(2.0 * inset_y),
                ),
            ));
        }
        cursor = next.max(cursor + 1);
    }
    rects
}

fn code_block(
    language: Option<&str>,
    code: &str,
    ix: usize,
    theme: &Theme,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    // Per line, so the block's height is exactly `lines × line height`. When
    // syntax highlighting arrives it recolors these runs and layout does not
    // move — highlight is pure paint.
    let mono = font(theme.font_mono.clone());
    let lines: Vec<AnyElement> = code
        .split('\n')
        .map(|line| {
            let run = TextRun {
                len: line.len(),
                font: mono.clone(),
                color: theme.text,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            StyledText::new(SharedString::from(line.to_string()))
                .with_runs(vec![run])
                .into_any_element()
        })
        .collect();

    div()
        .rounded(px(10.0))
        .bg(theme.ink(0.035))
        .border_1()
        .border_color(theme.border)
        .overflow_hidden()
        .relative()
        .when_some(language, |el, lang| {
            el.child(
                div()
                    .px(px(CODE_PADDING_X))
                    .py(px(5.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .bg(theme.ink(0.02))
                    .text_size(px(11.0))
                    .text_color(theme.text_muted)
                    .child(SharedString::from(lang.to_string())),
            )
        })
        .child(
            div()
                .id(ElementId::named_usize("md-code", ix))
                .overflow_x_scroll()
                .px(px(CODE_PADDING_X))
                .py(px(CODE_PADDING_Y))
                .text_size(px(CODE_TEXT_SIZE))
                .line_height(px(CODE_LINE_HEIGHT))
                .whitespace_nowrap()
                .children(lines),
        )
        .child(copy_button(code, ix, theme, window, cx))
        .into_any_element()
}

/// A copy button that owns its own feedback.
///
/// The state is the element's, not the caller's: a component library cannot ask
/// every host to thread a handler and a "which block is showing Copied" index
/// through its render tree just to put a button on a code block. It resets when
/// the pointer leaves, which needs no clock.
fn copy_button(
    code: &str,
    ix: usize,
    theme: &Theme,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let copied = window.use_keyed_state(ElementId::named_usize("md-copied", ix), cx, |_, _| false);
    let showing = *copied.read(cx);
    let text: SharedString = code.to_string().into();

    div()
        .id(ElementId::named_usize("md-copy", ix))
        .absolute()
        .top(px(3.0))
        .right(px(5.0))
        .h(px(20.0))
        .px(px(6.0))
        .rounded(px(5.0))
        .flex()
        .items_center()
        .cursor_pointer()
        .text_size(px(10.5))
        .text_color(theme.text_muted)
        .hover(|el| el.bg(theme.ink(0.08)))
        .child(if showing { "Copied" } else { "Copy" })
        .on_click({
            let copied = copied.clone();
            move |_, _, cx| {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.to_string()));
                copied.update(cx, |state, cx| {
                    *state = true;
                    cx.notify();
                });
            }
        })
        .on_hover(move |hovering, _, cx| {
            if !*hovering && *copied.read(cx) {
                copied.update(cx, |state, cx| {
                    *state = false;
                    cx.notify();
                });
            }
        })
        .into_any_element()
}

fn image(url: &str, alt: &str, theme: &Theme) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(
            div()
                .rounded(px(8.0))
                .overflow_hidden()
                .border_1()
                .border_color(theme.border)
                .child(img(SharedString::from(url.to_string())).max_w_full()),
        )
        .when(!alt.is_empty(), |el| {
            el.child(
                div()
                    .text_size(px(11.5))
                    .text_color(theme.text_muted)
                    .child(SharedString::from(alt.to_string())),
            )
        })
        .into_any_element()
}

/// A GFM table.
///
/// Columns are content-proportional with a per-column floor: each cell is
/// shaped unwrapped to get its max-content width, and the flex resolution does
/// the rest. When even the floors no longer fit, the table scrolls sideways
/// rather than crushing every column into per-character wrapping.
fn table(
    align: &[Align],
    header: &[Text],
    rows: &[Vec<Text>],
    ix: usize,
    theme: &Theme,
    window: &mut Window,
) -> AnyElement {
    let all: Vec<&[Text]> = std::iter::once(header)
        .filter(|row| !row.is_empty())
        .chain(rows.iter().map(|row| row.as_slice()))
        .collect();
    let columns = all.iter().map(|row| row.len()).max().unwrap_or(0);
    if columns == 0 {
        return gpui::Empty.into_any_element();
    }
    let has_header = !header.is_empty();

    let text_system = window.text_system();
    let mut flats: Vec<Vec<Option<Flat>>> = Vec::with_capacity(all.len());
    let mut content = vec![0.0f32; columns];
    for (r, row) in all.iter().enumerate() {
        let weight = if has_header && r == 0 {
            FontWeight::BOLD
        } else {
            FontWeight::NORMAL
        };
        let mut out = Vec::with_capacity(columns);
        for (c, natural) in content.iter_mut().enumerate() {
            let Some(cell) = row.get(c) else {
                out.push(None);
                continue;
            };
            let flat = flatten(cell, weight, theme);
            if !flat.text.is_empty() {
                let width = f32::from(
                    text_system
                        .shape_line(flat.text.clone(), px(TEXT_SIZE), &flat.runs, None)
                        .width(),
                );
                *natural = natural.max(width);
            }
            out.push(Some(flat));
        }
        flats.push(out);
    }

    let naturals: Vec<f32> = content
        .iter()
        .map(|width| width.max(TABLE_MIN_COLUMN_CONTENT) + 2.0 * TABLE_CELL_PADDING)
        .collect();
    let minimums: Vec<f32> = naturals
        .iter()
        .map(|natural| natural.min(TABLE_MIN_COLUMN_WIDTH))
        .collect();
    let hairline = theme.hairline(0.10);

    let mut inner = div()
        .flex()
        .flex_col()
        .w_full()
        .min_w(px(minimums.iter().sum::<f32>()));
    for (r, row) in flats.into_iter().enumerate() {
        if r > 0 {
            inner = inner.child(div().flex_none().h(px(TABLE_DIVIDER)).w_full().bg(hairline));
        }
        let mut row_el = div().flex().flex_row();
        for (c, cell) in row.into_iter().enumerate() {
            let mut cell_el = div()
                .flex_grow(naturals[c])
                .flex_shrink(naturals[c])
                .flex_basis(px(0.0))
                .min_w(px(minimums[c]))
                .p(px(TABLE_CELL_PADDING))
                .text_size(px(TEXT_SIZE))
                .line_height(px(LINE_HEIGHT));
            cell_el = match align.get(c).copied().unwrap_or_default() {
                Align::Left => cell_el,
                Align::Center => cell_el.text_center(),
                Align::Right => cell_el.text_right(),
            };
            if let Some(flat) = cell {
                let styled = StyledText::new(flat.text).with_runs(flat.runs);
                cell_el = cell_el.child(styled);
            }
            row_el = row_el.child(cell_el);
        }
        inner = inner.child(row_el);
    }

    div()
        .id(ElementId::named_usize("md-table", ix))
        .w_full()
        .overflow_x_scroll()
        .child(inner)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    #[test]
    fn flatten_cuts_the_text_at_every_mark_boundary() {
        let theme = Theme::dark();
        let doc = parse("plain **bold** `code` tail");
        let BlockKind::Paragraph(text) = &doc.blocks[0].kind else {
            panic!("expected a paragraph")
        };
        let flat = flatten(text, FontWeight::NORMAL, &theme);

        // The runs must cover the text exactly, or gpui shapes the wrong bytes.
        assert_eq!(
            flat.runs.iter().map(|run| run.len).sum::<usize>(),
            flat.text.len()
        );
        assert_eq!(flat.code.len(), 1);
        assert!(
            flat.runs
                .iter()
                .any(|run| run.font.weight == FontWeight::SEMIBOLD)
        );
    }

    #[test]
    fn runs_cover_the_text_for_every_shape_of_mark() {
        let theme = Theme::dark();
        for source in [
            "**_nested_**",
            "a [link](u) b",
            "![alt](u) trailing",
            "~~struck~~ and `mono`",
            "**bold `code` inside**",
            "no marks at all",
            "",
        ] {
            for block in parse(source).blocks {
                let Some(text) = block.text() else { continue };
                let flat = flatten(text, FontWeight::NORMAL, &theme);
                assert_eq!(
                    flat.runs.iter().map(|run| run.len).sum::<usize>(),
                    flat.text.len(),
                    "runs do not cover {source:?}"
                );
            }
        }
    }

    #[test]
    fn adjacent_links_merge_into_one_clickable_range() {
        let theme = Theme::dark();
        let doc = parse("[**bold** and plain](https://example.com)");
        let BlockKind::Paragraph(text) = &doc.blocks[0].kind else {
            panic!("expected a paragraph")
        };
        let flat = flatten(text, FontWeight::NORMAL, &theme);
        assert_eq!(flat.links.len(), 1);
        assert_eq!(flat.links[0].0, 0..text.text.len());
    }
}
