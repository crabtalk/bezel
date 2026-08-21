//! [`Doc`] → gpui elements.
//!
//! Numbers drive layout (sizes, line heights, paddings — the constants here);
//! colors are paint, read from [`Theme`]. Blocks are a flat list, so nesting is
//! left padding rather than nested containers, and the gap between two blocks
//! is decided by the pair: list items sit tight, everything else breathes.
//!
//! Ported from zeronsh/comet (MIT) and rebuilt against the flat block model.

use std::{cell::RefCell, ops::Range, rc::Rc};

use gpui::{
    AnyElement, App, BorderStyle, Bounds, CursorStyle, ElementId, FontStyle, FontWeight, Hsla,
    InteractiveText, ObjectFit, Pixels, Point, SharedString, StrikethroughStyle, StyledImage as _,
    StyledText, TextLayout, TextRun, UnderlineStyle, Window, canvas, div, font, img, point,
    prelude::*, px, quad, size,
};
use theme::Theme;

use crate::{
    doc::{Align, Block, BlockKind, Doc, Form, Mark, Part, Text},
    preview,
    select::{Cursor, Selection},
};

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
/// What a fence with no info string calls itself, in its header and in a
/// picker — one spelling, so the label and the menu row cannot disagree.
pub const PLAIN_LANGUAGE: &str = "Plain";
/// Inline code's wash is a rounded quad painted under the glyphs: a run's
/// `background_color` can only ever be a square box.
const INLINE_CODE_RADIUS: f32 = 4.5;
const INLINE_CODE_PAD_X: f32 = 2.0;
const INLINE_CODE_INSET_Y: f32 = 2.0;
/// A mention's chip — the same quad-under-glyphs trick as inline code, with
/// more room and an outline so the two do not read as the same thing.
const CHIP_RADIUS: f32 = 6.0;
const CHIP_PAD_X: f32 = 4.0;
const CHIP_INSET_Y: f32 = 1.0;
/// A chip with a block to itself is a real element rather than a wash, so it
/// has room for the favicon the inline one cannot hold.
const CHIP_BLOCK_PAD_X: f32 = 8.0;
const CHIP_BLOCK_PAD_Y: f32 = 3.0;
const CHIP_ICON: f32 = 15.0;
/// Bookmark metrics. Notion's card: 180px of image beside the text, and a
/// height that fits a title, two lines of blurb and a footer. A cover moves
/// that image above the text and gives it the card's full width.
const CARD_HEIGHT: f32 = 116.0;
const CARD_IMAGE_WIDTH: f32 = 180.0;
const CARD_COVER_HEIGHT: f32 = 200.0;
const CARD_PADDING: f32 = 14.0;
const CARD_TEXT_SIZE: f32 = 12.0;
const CARD_LINE_HEIGHT: f32 = 17.0;
const CARD_ICON: f32 = 16.0;
const CARD_COVER: f32 = 44.0;
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
pub struct BlockLayouts(Rc<RefCell<Frames>>);

#[derive(Default)]
struct Frames {
    texts: Vec<Painted>,
    /// Each block's whole box, which a text layout does not give: a rule and
    /// an image hold no text at all, and a gutter handle still has to find them.
    blocks: Vec<(usize, Bounds<Pixels>)>,
    /// A fenced block's language label, which a host may want to hang a
    /// picker on.
    languages: Vec<(usize, Bounds<Pixels>)>,
}

/// One shaped run and the slice of its part it covers.
///
/// A paragraph is one entry over all of its text; a code block is one entry per
/// line. The range is what lets both resolve a click the same way — the layout
/// answers in its own coordinates and the base puts the answer back into the
/// part's.
struct Painted {
    block: usize,
    part: Part,
    range: Range<usize>,
    layout: TextLayout,
}

impl BlockLayouts {
    /// The position under `point`.
    ///
    /// Falls back to the nearest text vertically, so clicking the margin
    /// beside a line — or below the last one — still lands somewhere useful
    /// rather than doing nothing.
    pub fn hit(&self, point: Point<Pixels>) -> Option<Cursor> {
        let entries = &self.0.borrow().texts;
        let cursor = |painted: &Painted| {
            let (Ok(offset) | Err(offset)) = painted.layout.index_for_position(point);
            Cursor::new(
                painted.block,
                painted.part,
                painted.range.start + offset.min(painted.range.len()),
            )
        };
        if let Some(painted) = entries
            .iter()
            .find(|painted| painted.layout.bounds().contains(&point))
        {
            return Some(cursor(painted));
        }
        entries
            .iter()
            .min_by_key(|painted| {
                let bounds = painted.layout.bounds();
                let above = (bounds.origin.y - point.y).abs();
                let below = (bounds.origin.y + bounds.size.height - point.y).abs();
                f32::from(above.min(below)) as i64
            })
            .map(cursor)
    }

    /// Where a position painted last frame, and how tall its line is.
    ///
    /// Vertical motion is geometry rather than arithmetic on line numbers, so
    /// a wrapped row and a hard newline are the same case and neither needs
    /// counting — the rule `ui::TextField` arrived at.
    pub fn position(&self, at: Cursor) -> Option<(Point<Pixels>, Pixels)> {
        let entries = &self.0.borrow().texts;
        let painted = entries.iter().find(|painted| {
            painted.block == at.block
                && painted.part == at.part
                && painted.range.start <= at.offset
                && at.offset <= painted.range.end
        })?;
        let point = painted
            .layout
            .position_for_index(at.offset - painted.range.start)?;
        Some((point, painted.layout.line_height()))
    }

    /// The position one painted row above or below `at`, and the row it landed
    /// on. Walks the recorded runs in paint order — which is document order.
    ///
    /// Two things make this refuse to be a hit test. The gap between blocks
    /// belongs to no run, so a probe there answers with whichever run is
    /// nearest — and at a boundary that is the block being *left*, whose bottom
    /// edge is zero pixels away while the next block's top is a whole gap. And
    /// `from` is passed in rather than derived from `at`, because an offset at
    /// a soft wrap belongs to two rows and `position_for_index` always answers
    /// with the first: derive it and every step down recomputes the same row.
    pub fn step_row(
        &self,
        at: Cursor,
        from: Point<Pixels>,
        down: bool,
    ) -> Option<(Cursor, Pixels)> {
        let entries = &self.0.borrow().texts;
        let ix = entries.iter().position(|painted| {
            painted.block == at.block
                && painted.part == at.part
                && painted.range.start <= at.offset
                && at.offset <= painted.range.end
        })?;
        let here = &entries[ix];
        let line = here.layout.line_height();
        let index_at = |painted: &Painted, y: Pixels| {
            let (Ok(offset) | Err(offset)) = painted.layout.index_for_position(point(from.x, y));
            (
                Cursor::new(
                    painted.block,
                    painted.part,
                    painted.range.start + offset.min(painted.range.len()),
                ),
                y,
            )
        };

        // A wrapped paragraph is one run holding several rows, so try to stay
        // inside it before looking for a neighbour.
        let bounds = here.layout.bounds();
        let target = if down { from.y + line } else { from.y - line };
        if target >= bounds.origin.y && target < bounds.origin.y + bounds.size.height {
            return Some(index_at(here, target));
        }

        let next = match down {
            true => entries.get(ix + 1)?,
            false => entries.get(ix.checked_sub(1)?)?,
        };
        // Enter the neighbour on the row facing the one just left.
        let bounds = next.layout.bounds();
        let row = match down {
            true => bounds.origin.y,
            false => bounds.origin.y + bounds.size.height - next.layout.line_height(),
        };
        Some(index_at(next, row))
    }

    /// Whether `point` is inside painted text.
    ///
    /// [`Self::hit`] answers with the nearest run wherever it is asked, which
    /// is what a click wants and what a *pointer* must not have: an I-beam
    /// belongs where a caret would land, not everywhere an editor's box
    /// reaches.
    pub fn over_text(&self, point: Point<Pixels>) -> bool {
        self.0
            .borrow()
            .texts
            .iter()
            .any(|painted| painted.layout.bounds().contains(&point))
    }

    /// The block under `point`, for a gutter handle and a drop target.
    pub fn block_at(&self, point: Point<Pixels>) -> Option<usize> {
        let blocks = &self.0.borrow().blocks;
        blocks
            .iter()
            .find(|(_, bounds)| bounds.contains(&point))
            .or_else(|| {
                blocks.iter().min_by_key(|(_, bounds)| {
                    let above = (bounds.origin.y - point.y).abs();
                    let below = (bounds.origin.y + bounds.size.height - point.y).abs();
                    f32::from(above.min(below)) as i64
                })
            })
            .map(|(ix, _)| *ix)
    }

    /// Where a block painted last frame, in window coordinates.
    pub fn block_bounds(&self, ix: usize) -> Option<Bounds<Pixels>> {
        self.0
            .borrow()
            .blocks
            .iter()
            .find(|(block, _)| *block == ix)
            .map(|(_, bounds)| *bounds)
    }

    /// Where a fenced block's language label painted — the box a host hangs its
    /// picker on. Recorded rather than derived: the word's width is the text
    /// system's answer, and padding arithmetic would be wrong the first time
    /// any of it changed.
    pub fn language_bounds(&self, ix: usize) -> Option<Bounds<Pixels>> {
        self.0
            .borrow()
            .languages
            .iter()
            .find(|(block, _)| *block == ix)
            .map(|(_, bounds)| *bounds)
    }

    fn record(&self, block: usize, part: Part, range: Range<usize>, layout: TextLayout) {
        self.0.borrow_mut().texts.push(Painted {
            block,
            part,
            range,
            layout,
        });
    }

    fn record_block(&self, ix: usize, bounds: Bounds<Pixels>) {
        self.0.borrow_mut().blocks.push((ix, bounds));
    }

    fn record_language(&self, ix: usize, bounds: Bounds<Pixels>) {
        self.0.borrow_mut().languages.push((ix, bounds));
    }

    fn clear(&self) {
        let mut frames = self.0.borrow_mut();
        frames.texts.clear();
        frames.blocks.clear();
        frames.languages.clear();
    }
}

/// What the editor needs painted into one text: which text it is, where the
/// caret sits, and where to record the layout a click resolves against.
///
/// One bundle rather than four parameters threaded through every block arm —
/// a read-only render builds it with no caret and no sink, and pays nothing.
#[derive(Clone, Copy)]
struct Overlay<'a> {
    block: usize,
    part: Part,
    selection: Option<Selection>,
    layouts: Option<&'a BlockLayouts>,
    /// Shown on the caret's block while it holds nothing. The renderer is the
    /// only thing that knows where that text sits, so the string comes to it.
    placeholder: Option<&'a SharedString>,
}

impl<'a> Overlay<'a> {
    fn at(self, part: Part) -> Self {
        Self { part, ..self }
    }

    fn here(&self) -> Cursor {
        Cursor::new(self.block, self.part, 0)
    }

    /// The caret's byte offset, if the head is in *this* text.
    fn caret(&self) -> Option<usize> {
        self.selection
            .map(|selection| selection.head)
            .filter(|head| head.block == self.block && head.part == self.part)
            .map(|head| head.offset)
    }

    /// The selected slice of this text, clipped to it.
    ///
    /// The comparison is on `(block, part)` alone: a selection covers this text
    /// entirely when it starts before and ends after, and the offsets only
    /// matter at the two ends.
    fn selected(&self, len: usize) -> Option<Range<usize>> {
        let selection = self.selection?;
        if selection.is_collapsed() {
            return None;
        }
        let (start, end) = selection.ordered();
        let here = self.here();
        let (first, last) = (
            Cursor::new(start.block, start.part, 0),
            Cursor::new(end.block, end.part, 0),
        );
        if here < first || here > last {
            return None;
        }
        let from = if here == first { start.offset } else { 0 };
        let to = if here == last { end.offset } else { len };
        (from < to).then_some(from..to.min(len))
    }

    /// Whether a block a caret cannot enter — a rule, an image — falls inside
    /// the selection, and so should show that it is going to be taken.
    fn covers_block(&self) -> bool {
        let Some(selection) = self.selection.filter(|s| !s.is_collapsed()) else {
            return false;
        };
        let (start, end) = selection.ordered();
        start.block < self.block && self.block < end.block
    }
}

/// Parse and render in one step — the common case for read-only content.
pub fn markdown(source: &str, window: &mut Window, cx: &mut App) -> AnyElement {
    render(&crate::parse(source), window, cx)
}

/// Render a document.
pub fn render(doc: &Doc, window: &mut Window, cx: &mut App) -> AnyElement {
    render_with_selection(doc, None, None, None, window, cx)
}

/// Render a document with a caret and a selection in it.
///
/// Both are paint-time concerns and nothing else: they read their positions off
/// the shaped text's own layout handle, the same way the inline-code wash does,
/// so nothing about layout depends on where the caret sits. An editor supplies
/// the selection and owns the focus and the keys; painting a caret and a few
/// quads is not worth a second renderer.
pub fn render_with_selection(
    doc: &Doc,
    selection: Option<Selection>,
    layouts: Option<&BlockLayouts>,
    placeholder: Option<SharedString>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    // Refilled every frame, in paint order — and emptied in *prepaint*, not
    // here. An editor reads last frame's positions while building this frame's
    // tree (a menu anchored at the caret, a handle beside a block), and
    // clearing at build time takes them away before it can. Placed first in the
    // column so it runs ahead of every recorder below it.
    let reset = layouts.map(|layouts| {
        let layouts = layouts.clone();
        canvas(move |_, _, _| layouts.clear(), |_, _, _, _| ())
            .absolute()
            .size(px(0.0))
    });
    // Cloned once so the theme is readable while `cx` stays free for the
    // element state the copy button needs.
    let theme = Theme::of(cx).clone();
    let mut column = div().flex().flex_col().children(reset);

    for (ix, block) in doc.blocks.iter().enumerate() {
        let gap = match doc.blocks.get(ix.wrapping_sub(1)) {
            None => 0.0,
            Some(previous) if tight(previous, block) => LIST_GAP,
            Some(_) => BLOCK_GAP,
        };
        let overlay = Overlay {
            block: ix,
            part: Part::Body,
            selection,
            layouts,
            placeholder: placeholder.as_ref(),
        };
        // The block's own box, recorded for a gutter handle and a drop target.
        // A rule and an image hold no text, so a layout would not find them.
        let frame = layouts.map(|layouts| {
            let layouts = layouts.clone();
            canvas(
                move |bounds, _, _| layouts.record_block(ix, bounds),
                |_, _, _, _| (),
            )
            .absolute()
            .size_full()
        });
        column = column.child(
            div()
                .mt(px(gap))
                .pl(px(block.indent as f32 * INDENT_WIDTH))
                .relative()
                .children(frame)
                // A block no caret can enter still has to show it is inside the
                // selection, or a rule between two paragraphs looks untouched
                // right up until it disappears.
                .when(overlay.covers_block() && block.parts().is_empty(), |el| {
                    el.rounded(px(4.0)).bg(theme.selection)
                })
                .child(block_element(block, overlay, &theme, window, cx)),
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

fn block_element(
    block: &Block,
    overlay: Overlay,
    theme: &Theme,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let body = overlay.at(Part::Body);
    match &block.kind {
        BlockKind::Paragraph(text) => text_element(
            text,
            TEXT_SIZE,
            LINE_HEIGHT,
            FontWeight::NORMAL,
            body,
            theme,
        ),
        BlockKind::Heading { level, text } => {
            let (size, line) = heading_metrics(*level);
            text_element(text, size, line, FontWeight::SEMIBOLD, body, theme)
        }
        BlockKind::Bullet(text) => marker_row(disc(theme), text, body, theme),
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
            body,
            theme,
        ),
        BlockKind::Task { checked, text } => {
            marker_row(checkbox(*checked, theme), text, body, theme)
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
                body,
                theme,
            ))
            .into_any_element(),
        BlockKind::Code { language, code } => code_block(
            language.as_deref(),
            &code.text,
            overlay.at(Part::Code),
            theme,
            window,
            cx,
        ),
        BlockKind::Image { url, alt } => image(url, alt, theme),
        BlockKind::Bookmark { url, form } => bookmark(overlay.block, url, *form, theme, cx),
        BlockKind::Table {
            align,
            header,
            rows,
        } => table(align, header, rows, overlay, theme, window),
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

fn marker_row(marker: AnyElement, text: &Text, overlay: Overlay, theme: &Theme) -> AnyElement {
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
            overlay,
            theme,
        )))
        .into_any_element()
}

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
            }
        }

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
        } else if italic {
            // Geist ships no italic face — its only variable axis is weight —
            // so an italic run asked of it paints upright.
            theme.font_sans_fallback.clone()
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
            // primary actions. A chip carries its own wash, so underlining it
            // too would say the same thing twice.
            color: if mono { theme.code_text } else { theme.text },
            background_color: None,
            underline: (link.is_some() && !chip).then_some(UnderlineStyle {
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

fn text_element(
    text: &Text,
    size: f32,
    line_height: f32,
    weight: FontWeight,
    overlay: Overlay,
    theme: &Theme,
) -> AnyElement {
    let flat = flatten(text, weight, theme);
    painted_text(flat, text.text.len(), size, line_height, overlay, theme)
}

/// Shaped inline content with the editing overlay under it: the selection, the
/// caret, the inline-code wash, and the layout a click resolves against.
///
/// Takes a [`Flat`] rather than a [`Text`] because a table has to shape every
/// cell to measure the columns before it can paint one.
fn painted_text(
    flat: Flat,
    len: usize,
    size: f32,
    line_height: f32,
    overlay: Overlay,
    theme: &Theme,
) -> AnyElement {
    let (ix, part) = (overlay.block, overlay.part);
    let (caret, selected) = (overlay.caret(), overlay.selected(len));
    let span = 0..len;
    // Only where the caret already is, and only while there is nothing to
    // read: a hint on every empty block would be a page of grey.
    let hint = overlay
        .placeholder
        .filter(|_| len == 0 && caret.is_some())
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
    let layouts = overlay.layouts.cloned();
    let underlay = canvas(
        |_, _, _| (),
        move |_, _, window, _| {
            if let Some(layouts) = &layouts {
                layouts.record(ix, part, span.clone(), layout.clone());
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
            // Wider, rounder and outlined, so a chip and an inline code span
            // never read as the same thing at a glance.
            for range in &chip_ranges {
                for rect in range_rects(&layout, range, CHIP_PAD_X, CHIP_INSET_Y) {
                    window.paint_quad(quad(
                        rect,
                        px(CHIP_RADIUS),
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
    overlay: Overlay,
    theme: &Theme,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let ix = overlay.block;
    // Per line, so the block's height is exactly `lines × line height`.
    // Highlighting recolors runs only — layout does not move, so a build with
    // no highlighter installed paints the same block in one plain run.
    let spans = crate::highlight::spans(cx, language, code);
    let mono = font(theme.font_mono.clone());
    let run = |len: usize, color: Hsla| TextRun {
        len,
        font: mono.clone(),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    // Each line's own layout, with the slice of the code it covers — the caret
    // and a click both resolve through these.
    let mut rows: Vec<(Range<usize>, TextLayout)> = Vec::new();
    let mut offset = 0usize;
    let lines: Vec<AnyElement> = code
        .split('\n')
        .map(|line| {
            let start = offset;
            offset += line.len() + 1;
            let mut runs = Vec::new();
            // Runs are measured within the line; spans are byte ranges over the
            // whole block, so every span is clipped to the line and rebased.
            let mut pos = 0usize;
            if let Some(spans) = &spans {
                let end = start + line.len();
                for (range, kind) in spans.iter().filter(|(r, _)| r.end > start && r.start < end) {
                    let s = range.start.clamp(start, end) - start;
                    let e = range.end.min(end) - start;
                    if s > pos {
                        runs.push(run(s - pos, theme.text));
                    }
                    runs.push(run(e - s, theme.syntax.color(*kind)));
                    pos = e;
                }
            }
            if pos < line.len() {
                runs.push(run(line.len() - pos, theme.text));
            }
            if runs.is_empty() {
                runs.push(run(0, theme.text));
            }
            let styled = StyledText::new(SharedString::from(line.to_string())).with_runs(runs);
            rows.push((start..start + line.len(), styled.layout().clone()));
            styled.into_any_element()
        })
        .collect();

    let caret = overlay.caret();
    let selected = overlay.selected(code.len());
    let sink = overlay.layouts.cloned();
    let (caret_color, selection_color) = (theme.caret, theme.selection);
    let underlay = canvas(
        |_, _, _| (),
        move |_, _, window, _| {
            for (span, layout) in &rows {
                if let Some(sink) = &sink {
                    sink.record(ix, Part::Code, span.clone(), layout.clone());
                }
                if let Some(range) = &selected {
                    let (from, to) = (range.start.max(span.start), range.end.min(span.end));
                    if from < to {
                        for rect in
                            range_rects(layout, &(from - span.start..to - span.start), 0.0, 0.0)
                        {
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
                }
                if let Some(offset) = caret.filter(|at| span.contains(at) || *at == span.end)
                    && let Some(head) = layout.position_for_index(offset - span.start)
                {
                    window.paint_quad(quad(
                        Bounds::new(head, size(px(1.5), layout.line_height())),
                        px(0.0),
                        caret_color,
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
        .rounded(px(10.0))
        .bg(theme.ink(0.035))
        .border_1()
        .border_color(theme.border)
        .overflow_hidden()
        .relative()
        // The band is unconditional: it is where the copy button already floats,
        // and where a host puts its language control — which needs somewhere to
        // sit on a block that has no language yet.
        .child(
            div()
                .relative()
                .flex()
                .flex_row()
                .items_center()
                .px(px(CODE_PADDING_X))
                .py(px(5.0))
                .border_b_1()
                .border_color(theme.border)
                .bg(theme.ink(0.02))
                .text_size(px(11.0))
                .text_color(match language {
                    Some(_) => theme.text_muted,
                    None => theme.text_faint,
                })
                // The label's own box, not the band's: a host hanging a picker
                // here wants it around the word, and only the word knows how
                // wide the word is.
                .child(
                    div()
                        .relative()
                        .children(overlay.layouts.map(|layouts| {
                            let layouts = layouts.clone();
                            canvas(
                                move |bounds, _, _| layouts.record_language(ix, bounds),
                                |_, _, _, _| (),
                            )
                            .absolute()
                            .size_full()
                        }))
                        .child(SharedString::from(
                            language.unwrap_or(PLAIN_LANGUAGE).to_string(),
                        )),
                ),
        )
        .child(
            div()
                .id(ElementId::named_usize("md-code", ix))
                .overflow_x_scroll()
                .relative()
                .px(px(CODE_PADDING_X))
                .py(px(CODE_PADDING_Y))
                .text_size(px(CODE_TEXT_SIZE))
                .line_height(px(CODE_LINE_HEIGHT))
                .whitespace_nowrap()
                .child(underlay)
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

/// A bookmark, in Notion's proportions: a fixed-height row with the text on the
/// left and an image panel of a fixed width on the right, all of it one click
/// target. [`Form::Embed`] turns the row into a column and gives the image the
/// card's full width instead, and [`Form::Chip`] is neither — a pill of favicon
/// and title, which is what an inline mention would be if shaped text had
/// anywhere to put a picture.
///
/// The text is a fixed height and its footer pinned to the bottom because a
/// preview resolves *after* the card has painted — a blurb arriving into a box
/// that grows would shove every block below it down the page.
fn bookmark(ix: usize, url: &str, form: Form, theme: &Theme, cx: &App) -> AnyElement {
    let preview = preview::of(cx, url).unwrap_or_default();
    let host = SharedString::from(preview::host(url).to_string());
    let label = preview.label.clone().unwrap_or_else(|| host.clone());
    let title = preview
        .title
        .clone()
        .unwrap_or_else(|| SharedString::from(url.to_string()));

    // Owned, because the image panel's fallback outlives this call: gpui asks
    // for the replacement element only once the fetch has failed.
    let (icon, muted, wash) = (preview.icon.clone(), theme.text_muted, theme.element_hover);
    let site = host.clone();
    let mark = move |size: f32| {
        let host = site.clone();
        match icon.clone() {
            Some(icon) => img(icon)
                .size(px(size))
                .rounded(px(size / 4.0))
                .with_fallback(move || initial(&host, size, muted, wash))
                .into_any_element(),
            None => initial(&host, size, muted, wash),
        }
    };

    if form == Form::Chip {
        let open = url.to_string();
        let pill = div()
            .id(ElementId::named_usize("md-chip", ix))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .px(px(CHIP_BLOCK_PAD_X))
            .py(px(CHIP_BLOCK_PAD_Y))
            .rounded(px(CHIP_RADIUS))
            .border_1()
            .border_color(theme.border)
            .bg(theme.element_hover)
            .text_size(px(TEXT_SIZE))
            .line_height(px(LINE_HEIGHT))
            .text_color(theme.text)
            .cursor(CursorStyle::PointingHand)
            .hover(|el| el.bg(theme.element_active))
            .on_click(move |_, _, cx| cx.open_url(&open))
            .child(mark(CHIP_ICON))
            // The host, not the URL, when nothing has resolved it: a chip is
            // the short form, and a raw URL in a pill is the long one.
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .child(preview.title.unwrap_or(label)),
            );
        // A block's own box is `display: block`, where a pill would take the
        // whole width. One flex row around it is what lets it hug its label.
        return div().flex().flex_row().child(pill).into_any_element();
    }

    let words = div()
        .flex()
        .flex_col()
        .min_w_0()
        .h(px(CARD_HEIGHT))
        .px(px(CARD_PADDING))
        .py(px(CARD_PADDING - 2.0))
        .child(
            div()
                .truncate()
                .text_size(px(TEXT_SIZE))
                .line_height(px(LINE_HEIGHT))
                .text_color(theme.text)
                .child(title),
        )
        .children(preview.description.map(|blurb| {
            div()
                .line_clamp(2)
                .text_size(px(CARD_TEXT_SIZE))
                .line_height(px(CARD_LINE_HEIGHT))
                .text_color(theme.text_muted)
                .child(blurb)
        }))
        .child(
            div()
                .mt_auto()
                .pt(px(6.0))
                .flex()
                .items_center()
                .gap(px(6.0))
                .text_size(px(CARD_TEXT_SIZE))
                .text_color(theme.text_muted)
                .child(mark(CARD_ICON))
                .child(div().truncate().child(label)),
        );

    let picture = div()
        .bg(theme.surface)
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .child(match preview.image {
            Some(image) => img(image)
                .size_full()
                .object_fit(ObjectFit::Cover)
                .with_fallback(move || mark(CARD_COVER))
                .into_any_element(),
            None => mark(CARD_COVER),
        });

    let open = url.to_string();
    let card = div()
        .id(ElementId::named_usize("md-bookmark", ix))
        .flex()
        .w_full()
        .overflow_hidden()
        .rounded(px(8.0))
        .border_1()
        .border_color(theme.border)
        .bg(theme.surface_card)
        .cursor(CursorStyle::PointingHand)
        .hover(|el| el.bg(theme.element_hover))
        .on_click(move |_, _, cx| cx.open_url(&open));

    if form == Form::Embed {
        card.flex_col()
            .child(picture.w_full().h(px(CARD_COVER_HEIGHT)))
            .child(words.w_full())
    } else {
        card.h(px(CARD_HEIGHT))
            .child(words.flex_1())
            .child(picture.flex_none().w(px(CARD_IMAGE_WIDTH)).h_full())
    }
    .into_any_element()
}

/// The mark a site gets before anyone has fetched its favicon: its host's first
/// letter, which is a placeholder no icon set has to ship.
fn initial(host: &str, size: f32, color: Hsla, wash: Hsla) -> AnyElement {
    div()
        .flex_none()
        .size(px(size))
        .rounded(px(size / 4.0))
        .bg(wash)
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(size * 0.55))
        .text_color(color)
        .child(SharedString::from(
            host.chars()
                .next()
                .unwrap_or('?')
                .to_uppercase()
                .to_string(),
        ))
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
    overlay: Overlay,
    theme: &Theme,
    window: &mut Window,
) -> AnyElement {
    let ix = overlay.block;
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
                // `all` drops an empty header, so a table without one starts at
                // part row 1 — row 0 is the header slot whether or not it is
                // filled.
                let row = if has_header { r } else { r + 1 };
                let len = flat.text.len();
                cell_el = cell_el.child(painted_text(
                    flat,
                    len,
                    TEXT_SIZE,
                    LINE_HEIGHT,
                    overlay.at(Part::Cell { row, column: c }),
                    theme,
                ));
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
