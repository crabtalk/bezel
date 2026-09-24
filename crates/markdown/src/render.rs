//! [`Doc`] → gpui elements.
//!
//! Numbers drive layout (sizes, line heights, paddings — the constants here);
//! colors are paint, read from [`Theme`]. Blocks are a flat list, so nesting is
//! left padding rather than nested containers, and the gap between two blocks
//! is decided by the pair: list items sit tight, everything else breathes.
//!
//! Ported from zeronsh/comet (MIT) and rebuilt against the flat block model.

use std::{cell::RefCell, ops::Range, path::Path, rc::Rc};

use gpui::{
    AnyElement, App, BorderStyle, Bounds, CursorStyle, ElementId, FontStyle, FontWeight, Hsla,
    ImageSource, InteractiveText, MouseButton, ObjectFit, Pixels, Point, SharedString,
    StrikethroughStyle, StyledImage as _, StyledText, TextLayout, TextRun, UnderlineStyle, Window,
    canvas, div, font, img, point, prelude::*, px, quad, size,
};
use theme::{TextStyle, Theme, Typeset};

use crate::{
    block,
    doc::{Align, Block, BlockKind, Doc, Form, Mark, Part, QuoteKind, Text},
    layout::Layout,
    preview,
    select::{Cursor, Selection},
    typography::Typography,
};

/// Space between two ordinary blocks, and the tighter space inside a list.
const BLOCK_GAP: f32 = 12.0;
const LIST_GAP: f32 = 4.0;
/// One indent level. Wide enough to clear a marker and read as a level.
const INDENT_WIDTH: f32 = 22.0;
/// The marker column of a list row.
const MARKER_WIDTH: f32 = 18.0;
const MARKER_GAP: f32 = 8.0;
/// What a fence holds its code in, inside its border.
const CODE_PADDING_X: f32 = 12.0;
const CODE_PADDING_Y: f32 = 10.0;
/// What a fence with no info string calls itself, in its header and in a
/// picker — one spelling, so the label and the menu row cannot disagree.
pub const PLAIN_LANGUAGE: &str = "Plain";
/// Width of the caret. Wider than a hairline, because it has to read at a
/// glance against the text it sits in.
const CARET_WIDTH: f32 = 1.5;
/// Inline code's wash is a rounded quad painted under the glyphs: a run's
/// `background_color` can only ever be a square box.
const INLINE_CODE_RADIUS: f32 = 4.5;
const INLINE_CODE_PAD_X: f32 = 2.0;
const INLINE_CODE_INSET_Y: f32 = 2.0;
/// A mention's chip — the same quad-under-glyphs trick as inline code, with
/// more room and an outline so the two do not read as the same thing.
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
const CARD_BORDER: f32 = 1.0;
const CARD_ICON: f32 = 16.0;
const CARD_COVER: f32 = 44.0;
/// Image metrics.
const IMAGE_EMPTY_HEIGHT: f32 = 52.0;
const CAPTION_GAP: f32 = 4.0;
/// What an image with no URL yet says, and what its caption says while empty.
const IMAGE_EMPTY: &str = "Add an image";
const CAPTION_HINT: &str = "Write a caption";
/// Table metrics. The design is frameless: hairlines between rows are the only
/// chrome — no outer box, no header fill, no rounding.
const TABLE_CELL_PADDING: f32 = 12.0;
const TABLE_DIVIDER: f32 = 1.0;
/// Floor for a column's max-content share, so a short column ("1k") beside a
/// prose column keeps a readable width.
const TABLE_MIN_COLUMN_CONTENT: f32 = 48.0;
/// Narrowest a column wraps down to before the table scrolls instead.
const TABLE_MIN_COLUMN_WIDTH: f32 = 96.0;

/// What an image's one authored string is doing on the page.
///
/// SwiftUI keeps three things apart — `accessibilityLabel` for a reader that
/// cannot see, `.help` for the pointer, and a caption you compose out of a
/// `Text` under the picture. Markdown has one slot for all three, so this says
/// which of them it is playing here rather than in the document, where it is
/// the same string either way.
///
/// A named choice rather than a `bool`, so a surface that wants a third answer
/// gets a variant instead of a second flag.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Caption {
    /// Under the picture, where a caret can sit in it. The editor's shape.
    #[default]
    Shown,
    /// Kept by the document and painted nowhere — a picture on its own.
    Hidden,
}

/// Whether a fence paints the button that copies its text.
///
/// A named choice rather than a `bool`, the way [`Caption`] is.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CopyButton {
    /// Floating at the top right of the band, on the pointer and off it.
    #[default]
    Shown,
    /// Painted nowhere. A document with this and no [`Editing::toggle`] holds
    /// no listener at all.
    Hidden,
}

/// A range the caller wants washed, and which wash it gets.
///
/// None of what a comment or a highlight *says* is here: the caller keeps it
/// and hands over the range, the way it hands over a [`crate::Preview`]. A
/// closed set rather than a color, so the environment keeps deciding the paint.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[non_exhaustive]
pub enum Annotation {
    /// A thread still waiting on someone.
    #[default]
    Open,
    /// Answered, and kept for the record.
    Resolved,
    /// The one whose thread the reader has in front of them.
    Active,
    /// A reader's highlight, in the wash [`crate::set_highlight_paint`]
    /// gives its colour.
    Highlight(crate::HighlightColor),
}

impl Annotation {
    fn wash(self, theme: &Theme, highlight: crate::HighlightPaint) -> Hsla {
        match self {
            Self::Open => theme.warning.opacity(0.20),
            Self::Resolved => theme.warning.opacity(0.08),
            Self::Active => theme.warning.opacity(0.38),
            Self::Highlight(color) => highlight(color, theme),
        }
    }
}

/// Handed the block whose checkbox was clicked — see [`Toggle::Handled`].
///
/// Shared rather than borrowed: the press listener it is cloned into outlives
/// the frame that built it.
pub type OnToggle = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// Who answers a press on a task block's checkbox.
///
/// Either variant paints the box as a control — the pointer over it is a hand.
/// [`Editing::toggle`] left unset paints it as a mark, and the press goes
/// wherever it would on any other glyph.
#[derive(Clone)]
pub enum Toggle {
    /// The box takes the press, stops it, and calls this with the block it
    /// belongs to. For a caller holding the [`Doc`] it renders itself.
    Handled(OnToggle),
    /// The box takes no press. The caller hit-tests
    /// [`BlockLayouts::checkbox_bounds`] in its own handler, which is what an
    /// editor does: the press it swallows is the one that also takes focus and
    /// closes an open menu, and the toggle belongs in the undo history beside
    /// the rest of its edits.
    HitTested,
}

/// What an editor paints over a document.
///
/// One value rather than six parameters, and the reason it is public: the
/// caret, the selection, the comment washes and the layout sink all arrive
/// together or not at all, and a read-only [`render`] sets none of them.
#[derive(Clone)]
pub struct Editing<'a> {
    /// The caret and what it has selected. `None` paints neither — a document
    /// nobody is editing.
    pub selection: Option<Selection>,
    /// The blink's lit half. A caret painted on every frame reads as frozen,
    /// and the phase belongs to whoever owns the focus.
    pub caret_on: bool,
    /// Filled as the document paints, for a caller resolving clicks against it.
    pub layouts: Option<&'a BlockLayouts>,
    /// Ranges washed under the text, in the order given.
    pub annotations: &'a [(Selection, Annotation)],
    /// Shown on the caret's block while it holds nothing.
    pub placeholder: Option<SharedString>,
    pub caption: Caption,
    /// What to set the document in. `None` takes the installed
    /// [`Typography`] — a caller sizing one document apart from the rest
    /// passes [`Typography::scaled`].
    pub typography: Option<Typography>,
    /// Makes a task block's checkbox a control, and says who answers the
    /// press. `None` paints a mark.
    pub toggle: Option<Toggle>,
    /// Whether a fence offers to copy itself.
    pub copy: CopyButton,
    /// The directory a relative image path is joined onto. `None` leaves it
    /// relative, which gpui reads against the process's working directory.
    pub base: Option<&'a Path>,
}

impl Default for Editing<'_> {
    fn default() -> Self {
        Self {
            selection: None,
            // Lit, so that a caller setting a selection and nothing else gets a
            // caret rather than a mystery.
            caret_on: true,
            layouts: None,
            annotations: &[],
            placeholder: None,
            caption: Caption::default(),
            typography: None,
            toggle: None,
            copy: CopyButton::default(),
            base: None,
        }
    }
}

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
    rows: Vec<PaintedRow>,
    /// Each block's whole box, which a text layout does not give: a rule and
    /// an image hold no text at all, and a gutter handle still has to find them.
    blocks: Vec<(usize, Bounds<Pixels>)>,
    /// A fenced block's language label, which a host may want to hang a
    /// picker on.
    languages: Vec<(usize, Bounds<Pixels>)>,
    /// An image block's picture, which is not its block: the block runs the
    /// full column and carries the caption, and a resize handle belongs on the
    /// edge of the picture itself.
    pictures: Vec<(usize, Bounds<Pixels>)>,
    /// A task block's checkbox, which is not its marker column: the column is
    /// gutter either side of the box, and a click there places a caret.
    checkboxes: Vec<(usize, Bounds<Pixels>)>,
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

struct PaintedRow {
    painted: usize,
    block: usize,
    part: Part,
    range: Range<usize>,
    bounds: Bounds<Pixels>,
    line_start: usize,
    wrapped_row: usize,
}

impl BlockLayouts {
    /// The position under `point`.
    ///
    /// Falls back to the nearest text vertically, so clicking the margin
    /// beside a line — or below the last one — still lands somewhere useful
    /// rather than doing nothing.
    pub fn hit(&self, point: Point<Pixels>) -> Option<Cursor> {
        let frames = self.0.borrow();
        if let Some(row) = frames.rows.iter().find(|row| row.bounds.contains(&point)) {
            return cursor_in_row(&frames, row, point.x);
        }
        frames
            .rows
            .iter()
            .min_by_key(|row| {
                let bounds = row.bounds;
                let above = (bounds.origin.y - point.y).abs();
                let below = (bounds.origin.y + bounds.size.height - point.y).abs();
                f32::from(above.min(below)) as i64
            })
            .and_then(|row| cursor_in_row(&frames, row, point.x))
    }

    /// Where a position painted last frame, and how tall its line is.
    ///
    /// Vertical motion is geometry rather than arithmetic on line numbers, so
    /// a wrapped row and a hard newline are the same case and neither needs
    /// counting — the rule `ui::TextField` arrived at.
    pub fn position(&self, at: Cursor) -> Option<(Point<Pixels>, Pixels)> {
        let frames = self.0.borrow();
        let painted = frames.texts.iter().find(|painted| {
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

    /// The painted rows of a range, in document order — what a bar centred over
    /// a selection is placed against, and what a highlight of a caller's own is
    /// drawn from.
    ///
    /// One rect per visual row rather than one box: a selection that wraps or
    /// crosses blocks has no single box, and a caller given one would paint
    /// over the gaps.
    pub fn rects(&self, selection: Selection) -> Vec<Bounds<Pixels>> {
        let (start, end) = selection.ordered();
        self.0
            .borrow()
            .texts
            .iter()
            .filter_map(|painted| {
                let here = Cursor::new(painted.block, painted.part, 0);
                let (from, to) = (
                    Cursor::new(start.block, start.part, 0),
                    Cursor::new(end.block, end.part, 0),
                );
                if here < from || here > to {
                    return None;
                }
                // The offsets only matter at the two ends: in between, the
                // whole of the text is covered.
                let len = painted.range.len();
                let first = if here == from { start.offset } else { 0 };
                let last = if here == to { end.offset } else { usize::MAX };
                let range = first.saturating_sub(painted.range.start).min(len)
                    ..last.saturating_sub(painted.range.start).min(len);
                (range.start < range.end).then(|| range_rects(&painted.layout, &range, 0.0, 0.0))
            })
            .flatten()
            .collect()
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
        let frames = self.0.borrow();
        let ix = frames
            .rows
            .iter()
            .position(|row| {
                row.block == at.block
                    && row.part == at.part
                    && row_contains(row, at.offset)
                    && row.bounds.origin.y <= from.y
                    && from.y < row.bounds.origin.y + row.bounds.size.height
            })
            .or_else(|| {
                frames.rows.iter().position(|row| {
                    row.block == at.block && row.part == at.part && row_contains(row, at.offset)
                })
            })?;
        let next = match down {
            true => frames.rows.get(ix + 1)?,
            false => frames.rows.get(ix.checked_sub(1)?)?,
        };
        Some((cursor_in_row(&frames, next, from.x)?, next.bounds.origin.y))
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

    /// Where a block's first painted row sits, and how tall that row is — what
    /// a mark in the gutter has to line up with.
    ///
    /// [`Self::block_bounds`] is not that: it spans every row the block holds,
    /// and a heading's single row is taller than a paragraph's, so anything
    /// placed from the top of the box rides above the text it points at.
    /// `None` for a block that paints no text at all, a rule being the one
    /// that does.
    pub fn first_row(&self, ix: usize) -> Option<(Pixels, Pixels)> {
        let texts = &self.0.borrow().texts;
        let painted = texts.iter().find(|painted| painted.block == ix)?;
        Some((
            painted.layout.bounds().origin.y,
            painted.layout.line_height(),
        ))
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

    /// Where an image block's picture painted, which
    /// [`BlockLayouts::block_bounds`] does not give: that box spans the column
    /// and takes in the caption, so a handle placed from it sits off the edge
    /// of any picture narrower than the page.
    pub fn picture_bounds(&self, ix: usize) -> Option<Bounds<Pixels>> {
        self.0
            .borrow()
            .pictures
            .iter()
            .find(|(block, _)| *block == ix)
            .map(|(_, bounds)| *bounds)
    }

    /// Where a task block's checkbox painted, which is the box itself and not
    /// the marker column it sits in — a press outside it is a press on the
    /// gutter, and belongs to whatever handles one.
    pub fn checkbox_bounds(&self, ix: usize) -> Option<Bounds<Pixels>> {
        self.0
            .borrow()
            .checkboxes
            .iter()
            .find(|(block, _)| *block == ix)
            .map(|(_, bounds)| *bounds)
    }

    fn record(&self, block: usize, part: Part, range: Range<usize>, layout: TextLayout) {
        let mut frames = self.0.borrow_mut();
        let painted = frames.texts.len();
        record_rows(&mut frames.rows, painted, block, part, &range, &layout);
        frames.texts.push(Painted {
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

    fn record_picture(&self, ix: usize, bounds: Bounds<Pixels>) {
        self.0.borrow_mut().pictures.push((ix, bounds));
    }

    fn record_checkbox(&self, ix: usize, bounds: Bounds<Pixels>) {
        self.0.borrow_mut().checkboxes.push((ix, bounds));
    }

    fn clear(&self) {
        let mut frames = self.0.borrow_mut();
        frames.texts.clear();
        frames.rows.clear();
        frames.blocks.clear();
        frames.languages.clear();
        frames.pictures.clear();
        frames.checkboxes.clear();
    }
}

fn row_contains(row: &PaintedRow, offset: usize) -> bool {
    row.range.start <= offset && offset <= row.range.end
}

fn cursor_in_row(frames: &Frames, row: &PaintedRow, x: Pixels) -> Option<Cursor> {
    let painted = &frames.texts[row.painted];
    let line = painted
        .layout
        .line_layout_for_index(row.line_start - painted.range.start)?;
    let height = row.bounds.size.height;
    let local = point(
        x - row.bounds.origin.x,
        height * (row.wrapped_row as f32 + 0.5),
    );
    let (Ok(offset) | Err(offset)) = line.closest_index_for_position(local, height);
    Some(Cursor::new(
        row.block,
        row.part,
        (row.line_start + offset).min(row.range.end),
    ))
}

fn record_rows(
    rows: &mut Vec<PaintedRow>,
    painted: usize,
    block: usize,
    part: Part,
    range: &Range<usize>,
    layout: &TextLayout,
) {
    let line_height = layout.line_height();
    let bounds = layout.bounds();
    let mut origin = bounds.origin;
    let mut line_start = range.start;
    for line in layout.line_layouts() {
        let shaped = &line.unwrapped_layout;
        let row_ends = line
            .wrap_boundaries()
            .iter()
            .map(|wrap| shaped.runs[wrap.run_ix].glyphs[wrap.glyph_ix].index)
            .chain([line.len()]);
        let mut row_start = 0;
        for (row, row_end) in row_ends.enumerate() {
            rows.push(PaintedRow {
                painted,
                block,
                part,
                range: line_start + row_start..line_start + row_end,
                bounds: Bounds::new(
                    origin + point(px(0.0), line_height * row as f32),
                    size(bounds.size.width, line_height),
                ),
                line_start,
                wrapped_row: row,
            });
            row_start = row_end;
        }
        origin.y += line.size(line_height).height;
        line_start += line.len() + 1;
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
    caret_on: bool,
    layouts: Option<&'a BlockLayouts>,
    /// Ranges washed under the text, in the order the caller gave them.
    annotations: &'a [(Selection, Annotation)],
    /// Shown on the caret's block while it holds nothing. The renderer is the
    /// only thing that knows where that text sits, so the string comes to it.
    placeholder: Option<&'a SharedString>,
    caption: Caption,
    /// Borrowed so [`Overlay`] stays `Copy` — the clone is made at the one
    /// press listener that needs an owned handle.
    toggle: Option<&'a Toggle>,
    copy: CopyButton,
    base: Option<&'a Path>,
    highlight: crate::HighlightPaint,
}

impl<'a> Overlay<'a> {
    fn at(self, part: Part) -> Self {
        Self { part, ..self }
    }

    fn here(&self) -> Cursor {
        Cursor::new(self.block, self.part, 0)
    }

    /// The caret to paint: where it is, and only on the blink's lit half.
    ///
    /// Separate from [`Self::caret`] because the blink must not reach anything
    /// but the quad — a block whose paint depends on holding the caret would
    /// otherwise swap itself out twice a second.
    fn caret_painted(&self) -> Option<usize> {
        self.caret_on.then(|| self.caret()).flatten()
    }

    /// The caret's byte offset, if the head is in *this* text.
    fn caret(&self) -> Option<usize> {
        self.selection
            .map(|selection| selection.head)
            .filter(|head| head.block == self.block && head.part == self.part)
            .map(|head| head.offset)
    }

    /// The selected slice of this text, clipped to it.
    fn selected(&self, len: usize) -> Option<Range<usize>> {
        self.clip(self.selection?, len)
    }

    /// The annotated slices of this text, already resolved to their paint —
    /// the wash goes into a `move` closure that the theme does not travel into.
    fn annotated(&self, len: usize, theme: &Theme) -> Vec<(Range<usize>, Hsla)> {
        self.annotations
            .iter()
            .filter_map(|(range, kind)| {
                Some((self.clip(*range, len)?, kind.wash(theme, self.highlight)))
            })
            .collect()
    }

    /// A range clipped to this text, and `None` when it does not reach it.
    ///
    /// The comparison is on `(block, part)` alone: a range covers this text
    /// entirely when it starts before and ends after, and the offsets only
    /// matter at the two ends.
    fn clip(&self, selection: Selection, len: usize) -> Option<Range<usize>> {
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

    /// Whether a block painting something a caret cannot enter — a rule, a
    /// picture — falls inside the selection, and so should show that it is
    /// going to be taken.
    fn covers_block(&self) -> bool {
        let Some(selection) = self.selection.filter(|s| !s.is_collapsed()) else {
            return false;
        };
        let (start, end) = selection.ordered();
        start.block < self.block && self.block < end.block
    }
}

/// What gpui loads for an image URL as written in a document.
///
/// Anything with `://` is fetched as it stands. Anything else is a file: an
/// absolute path as it stands, a relative one joined onto `base` when there is
/// one.
pub fn image_source(url: &str, base: Option<&Path>) -> ImageSource {
    if url.contains("://") {
        return SharedString::from(url.to_string()).into();
    }
    // gpui reads a file only from a `PathBuf` — handed a string it looks for
    // an asset built into the binary and paints nothing.
    match base {
        Some(base) => base.join(url).into(),
        None => std::path::PathBuf::from(url).into(),
    }
}

/// Parse and render in one step — the common case for read-only content.
pub fn markdown(source: &str, window: &mut Window, cx: &mut App) -> AnyElement {
    let doc = crate::parse_with(source, &crate::Marks::of(cx));
    render(&doc, Caption::default(), window, cx)
}

/// Render a document.
pub fn render(doc: &Doc, caption: Caption, window: &mut Window, cx: &mut App) -> AnyElement {
    render_with(
        doc,
        Editing {
            caption,
            ..Editing::default()
        },
        window,
        cx,
    )
}

/// Render a document with a caret and a selection in it.
///
/// Both are paint-time concerns and nothing else: they read their positions off
/// the shaped text's own layout handle, the same way the inline-code wash does,
/// so nothing about layout depends on where the caret sits. An editor supplies
/// the selection and owns the focus and the keys; painting a caret and a few
/// quads is not worth a second renderer.
pub fn render_with(doc: &Doc, editing: Editing, window: &mut Window, cx: &mut App) -> AnyElement {
    let Editing {
        selection,
        caret_on,
        layouts,
        annotations,
        placeholder,
        caption,
        typography,
        toggle,
        copy,
        base,
    } = editing;
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
    let typography = typography.unwrap_or_else(|| Typography::of(cx));
    let highlight = crate::marks::highlight_paint_of(cx);
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
            caret_on,
            layouts,
            annotations,
            placeholder: placeholder.as_ref(),
            caption,
            toggle: toggle.as_ref(),
            copy,
            base,
            highlight,
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
            // The indent sits on the outside and the recorder on the inside,
            // so what is recorded is the box the block's text actually
            // occupies. Recorded outside the padding, every level answered
            // with the same left edge, and a gutter handle placed from it
            // stayed at the margin while the block it belongs to moved right.
            div()
                .mt(px(gap))
                .pl(px(block.indent as f32 * INDENT_WIDTH))
                .child(
                    div()
                        .w_full()
                        .relative()
                        .children(frame)
                        // What a caret cannot enter still has to show it is
                        // inside the selection, or a rule between two
                        // paragraphs looks untouched right up until it
                        // disappears.
                        .when(overlay.covers_block() && block.opaque(), |el| {
                            el.rounded(px(4.0)).bg(theme.selection)
                        })
                        .child(block_element(
                            block,
                            overlay,
                            &typography,
                            &theme,
                            window,
                            cx,
                        )),
                ),
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
    typography: &Typography,
    theme: &Theme,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let body = overlay.at(Part::Body);
    match &block.kind {
        BlockKind::Paragraph(text) => text_element(
            text,
            typography.body.size(),
            typography.body.line_height(),
            FontWeight::NORMAL,
            body,
            theme,
            cx,
        ),
        BlockKind::Heading { level, text } => {
            let heading = typography.heading(*level);
            text_element(
                text,
                heading.size(),
                heading.line_height(),
                heading.weight,
                body,
                theme,
                cx,
            )
        }
        BlockKind::Bullet(text) => {
            marker_row(disc(typography, theme), text, body, typography, theme, cx)
        }
        BlockKind::Ordered { number, text } => marker_row(
            div()
                .flex_none()
                .w(px(MARKER_WIDTH))
                .text_size(px(typography.body.size()))
                .line_height(px(typography.body.line_height()))
                .text_color(theme.text_muted)
                .child(SharedString::from(format!("{number}.")))
                .into_any_element(),
            text,
            body,
            typography,
            theme,
            cx,
        ),
        BlockKind::Task { checked, text } => marker_row(
            checkbox(*checked, overlay, typography, theme),
            text,
            body,
            typography,
            theme,
            cx,
        ),
        BlockKind::Quote { kind, text } => div()
            .border_l_2()
            .border_color(kind.map_or(theme.border_strong, |kind| alert_color(kind, theme)))
            .pl(px(12.0))
            .pr(px(10.0))
            .py(px(2.0))
            .text_color(theme.text_muted)
            .children(kind.map(|kind| {
                div()
                    .pb(px(2.0))
                    .text_size(px(typography.body.size()))
                    .line_height(px(typography.body.line_height()))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(alert_color(kind, theme))
                    .child(kind.label())
            }))
            .child(text_element(
                text,
                typography.body.size(),
                typography.body.line_height(),
                FontWeight::NORMAL,
                body,
                theme,
                cx,
            ))
            .into_any_element(),
        BlockKind::Code { language, code } => {
            let overlay = overlay.at(Part::Code);
            // The caret in the fence gives the source back. A painted block is
            // still an editable one, and typing into it otherwise edits what
            // the reader cannot see.
            let painted = overlay
                .caret()
                .is_none()
                .then(|| block::render(language.as_deref(), &code.text, window, cx))
                .flatten();
            match painted {
                // Painted, there is no text under the selection to carry it —
                // the wash an opaque block gets at the container comes here.
                Some(element) => div()
                    .when(overlay.covers_block(), |el| {
                        el.rounded(px(4.0)).bg(theme.selection)
                    })
                    .child(element)
                    .into_any_element(),
                None => code_block(
                    language.as_deref(),
                    &code.text,
                    overlay,
                    typography,
                    theme,
                    window,
                    cx,
                ),
            }
        }
        BlockKind::Image { url, alt, width } => {
            image(url, alt, *width, overlay, typography, theme, cx)
        }
        BlockKind::Bookmark { url, form } => {
            bookmark(overlay.block, url, *form, typography, theme, cx)
        }
        BlockKind::Table {
            align,
            header,
            rows,
        } => table(align, header, rows, overlay, typography, theme, window, cx),
        BlockKind::Rule => div()
            .h(px(1.0))
            .w_full()
            .bg(theme.border)
            .into_any_element(),
    }
}

/// A real 5px disc rather than the "•" glyph, which reads too small at body size.
fn disc(typography: &Typography, theme: &Theme) -> AnyElement {
    div()
        .flex_none()
        .w(px(MARKER_WIDTH))
        .h(px(typography.body.line_height()))
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

fn checkbox(checked: bool, overlay: Overlay, typography: &Typography, theme: &Theme) -> AnyElement {
    let ix = overlay.block;
    let mut box_ = div()
        .relative()
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
            .text_style(TextStyle::Caption)
            .text_color(theme.on_solid)
            .child("✓")
    } else {
        box_.border_color(theme.border_strong)
    };
    // The box's own bounds rather than the marker column's: a caller hit-tests
    // these to tell a toggle from a caret placed in the gutter beside it.
    box_ = box_.children(overlay.layouts.map(|layouts| {
        let layouts = layouts.clone();
        canvas(
            move |bounds, _, _| layouts.record_checkbox(ix, bounds),
            |_, _, _, _| (),
        )
        .absolute()
        .size_full()
    }));
    // The cursor answers to either variant: a box an editor hit-tests is as
    // pressable as one the renderer listens to, and only the pointer says so.
    if overlay.toggle.is_some() {
        box_ = box_.cursor_pointer();
    }
    if let Some(Toggle::Handled(toggle)) = overlay.toggle.cloned() {
        box_ = box_.on_mouse_down(MouseButton::Left, move |_, window, cx| {
            // Stopped, or the press goes on to whatever placed a caret
            // under it and the toggle reads as a click that moved the
            // caret as well.
            cx.stop_propagation();
            toggle(ix, window, cx);
        });
    }

    div()
        .flex_none()
        .w(px(MARKER_WIDTH))
        .h(px(typography.body.line_height()))
        .flex()
        .items_center()
        .child(box_)
        .into_any_element()
}

/// What an alert paints its rule and its label in.
fn alert_color(kind: QuoteKind, theme: &Theme) -> Hsla {
    match kind {
        QuoteKind::Note => theme.accent,
        QuoteKind::Tip => theme.success,
        QuoteKind::Important => theme.busy,
        QuoteKind::Warning => theme.warning,
        QuoteKind::Caution => theme.danger,
    }
}

fn marker_row(
    marker: AnyElement,
    text: &Text,
    overlay: Overlay,
    typography: &Typography,
    theme: &Theme,
    cx: &App,
) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .gap(px(MARKER_GAP))
        .child(marker)
        .child(div().flex_1().min_w_0().child(text_element(
            text,
            typography.body.size(),
            typography.body.line_height(),
            FontWeight::NORMAL,
            overlay,
            theme,
            cx,
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

fn text_element(
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
fn painted_text(
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
fn caret_quad(head: Point<Pixels>, size: f32, line_height: Pixels) -> Bounds<Pixels> {
    let inset = (line_height - px(size)) / 2.0;
    Bounds::new(
        head + point(px(0.0), inset),
        gpui::size(px(CARET_WIDTH), px(size)),
    )
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

/// Paint a document's own markdown source: a fence's caret, selection and hit
/// testing, without a fence's box, band or copy button.
///
/// The caret is a [`Cursor`] at block 0 in [`Part::Code`] — what a document
/// held as one fence answers to, which is how an editor holds its source.
/// Wrapping is not optional here: a paragraph is one line of markdown, and a
/// source view that scrolled sideways would hide most of it.
pub fn render_source(code: &str, editing: Editing, cx: &mut App) -> AnyElement {
    let Editing {
        selection,
        caret_on,
        layouts,
        annotations,
        typography,
        ..
    } = editing;
    // The same reset `render_with` opens with, and for the same reason: the
    // positions this frame records are the ones the next click resolves
    // against, and last frame's have to go first.
    let reset = layouts.map(|layouts| {
        let layouts = layouts.clone();
        canvas(move |_, _, _| layouts.clear(), |_, _, _, _| ())
            .absolute()
            .size(px(0.0))
    });
    let theme = Theme::of(cx).clone();
    let typography = typography.unwrap_or_else(|| Typography::of(cx));
    let overlay = Overlay {
        block: 0,
        part: Part::Code,
        selection,
        caret_on,
        layouts,
        annotations,
        placeholder: None,
        caption: Caption::default(),
        // The source view is one fence and holds no task block.
        toggle: None,
        // It paints no band, so there is nowhere for the button to float.
        copy: CopyButton::Hidden,
        base: None,
        highlight: crate::marks::highlight_paint_of(cx),
    };
    let (underlay, lines) = code_lines(
        Some(crate::source::LANGUAGES[0]),
        code,
        overlay,
        &typography,
        &theme,
        cx,
    );
    // Keep each number beside its source line, including wrapped and empty lines.
    let style = crate::SourceStyle::of(cx);
    let digits = lines.len().to_string().len().max(style.gutter_min_digits);
    let gap = style.gutter_gap.max(0.0) * typography.code.size();
    let gutter_width = digits as f32 * typography.code.size() + gap;
    let lines = lines
        .into_iter()
        .enumerate()
        .map(|(index, line)| {
            if !style.line_numbers {
                return line;
            }
            div()
                .flex()
                .items_start()
                .child(
                    div()
                        .w(px(gutter_width))
                        .flex_shrink_0()
                        .pr(px(gap))
                        .font_family(theme.font_mono.clone())
                        .text_color(style.gutter_color.unwrap_or(theme.text_faint))
                        .text_right()
                        .child((index + 1).to_string()),
                )
                .child(div().flex_1().min_w_0().child(line))
                .into_any_element()
        })
        .collect();
    div()
        .flex()
        .flex_col()
        .children(reset)
        .child(code_body(0, underlay, lines, &typography, true))
        .into_any_element()
}

/// The shaped lines of a fence, and the canvas that paints the caret, the
/// selection and the annotations over them.
fn code_lines(
    language: Option<&str>,
    code: &str,
    overlay: Overlay,
    typography: &Typography,
    theme: &Theme,
    cx: &App,
) -> (AnyElement, Vec<AnyElement>) {
    let ix = overlay.block;
    // Highlighting recolors runs only — layout does not move, so a build with
    // no highlighter installed paints the same block in one plain run.
    // Markdown is the one language this crate can colour on its own, which is
    // what a source view is painted with where no highlighter reaches.
    let spans = crate::highlight::spans(cx, language, code).or_else(|| {
        language
            .filter(|language| crate::source::is_markdown(language))
            .map(|_| crate::source::spans(code))
    });
    let mono = font(theme.font_mono.clone());
    let run = |len: usize, color: Hsla| TextRun {
        len,
        font: mono.clone(),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    // Each source line's own layout, with the slice of the code it covers —
    // the caret and a click both resolve through these. A wrapped line is
    // several rows of one layout, which is the case `range_rects` already
    // walks for a paragraph.
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

    let caret = overlay.caret_painted();
    let selected = overlay.selected(code.len());
    let sink = overlay.layouts.cloned();
    let code_size = typography.code.size();
    let annotated = overlay.annotated(code.len(), theme);
    let (caret_color, selection_color) = (theme.caret, theme.selection);
    let underlay = canvas(
        |_, _, _| (),
        move |_, _, window, _| {
            for (span, layout) in &rows {
                if let Some(sink) = &sink {
                    sink.record(ix, Part::Code, span.clone(), layout.clone());
                }
                for (range, wash) in &annotated {
                    let (from, to) = (range.start.max(span.start), range.end.min(span.end));
                    if from < to {
                        for rect in
                            range_rects(layout, &(from - span.start..to - span.start), 0.0, 0.0)
                        {
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
                        caret_quad(head, code_size, layout.line_height()),
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

    (underlay.into_any_element(), lines)
}

fn code_block(
    language: Option<&str>,
    code: &str,
    overlay: Overlay,
    typography: &Typography,
    theme: &Theme,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let ix = overlay.block;
    let (underlay, lines) = code_lines(language, code, overlay, typography, theme, cx);
    let body = code_body(ix, underlay, lines, typography, Layout::of(cx).wrap_code);

    div()
        .rounded(px(Theme::panel_radius()))
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
                .text_style(TextStyle::Subheadline)
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
        .child(body)
        .children(
            (overlay.copy == CopyButton::Shown).then(|| copy_button(code, ix, theme, window, cx)),
        )
        .into_any_element()
}

/// The lines of a fence, wrapped to the block or scrolling sideways under it.
fn code_body(
    ix: usize,
    underlay: AnyElement,
    lines: Vec<AnyElement>,
    typography: &Typography,
    wrap: bool,
) -> AnyElement {
    let column = div()
        .flex()
        .flex_col()
        .px(px(CODE_PADDING_X))
        .children(lines);
    let body = div()
        .id(ElementId::named_usize("md-code", ix))
        .relative()
        .py(px(CODE_PADDING_Y))
        .text_size(px(typography.code.size()))
        .line_height(px(typography.code.line_height()))
        .child(underlay);
    if wrap {
        // The column is the block's width here rather than its widest line's,
        // which is what gives the text something to wrap against.
        body.child(column.w_full()).into_any_element()
    } else {
        ui::scroll::Viewport::new(
            format!("md-code-scroll-{ix}"),
            body.flex()
                .flex_row()
                .whitespace_nowrap()
                // The padding belongs to the lines, not to the scroller: a scroll
                // container's trailing padding is not part of what it will scroll
                // to, so the last characters of a long line sit behind the right
                // edge with nowhere left to go. As a row's only item this column is
                // sized by its widest line, and the padding rides along inside that
                // width.
                .child(column.items_start()),
            gpui::Axis::Horizontal,
        )
        .into_any_element()
    }
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
        .text_style(TextStyle::Caption)
        .text_color(theme.text_muted)
        .hover(|el| el.bg(theme.element_hover))
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

/// A picture and the caption under it, which is the alt text a caret can reach.
///
/// The caption row appears when there is something to read or somewhere to
/// type, so a document being read is not a column of pictures each trailing a
/// blank line. With no URL yet the picture is a dashed row instead — the shape
/// the slash menu makes, waiting to be told what to show.
fn image(
    url: &str,
    alt: &Text,
    width: Option<u32>,
    overlay: Overlay,
    typography: &Typography,
    theme: &Theme,
    cx: &App,
) -> AnyElement {
    let hint = SharedString::new_static(CAPTION_HINT);
    let overlay = Overlay {
        placeholder: Some(&hint),
        ..overlay.at(Part::Caption)
    };
    let picture = if url.is_empty() {
        div()
            .h(px(IMAGE_EMPTY_HEIGHT))
            .flex()
            .items_center()
            .px(px(CARD_PADDING))
            .rounded(px(Theme::button_radius()))
            .border_1()
            .border_dashed()
            .border_color(theme.border)
            .text_size(px(typography.body.size()))
            .text_color(theme.text_muted)
            .child(IMAGE_EMPTY)
    } else {
        let picture = img(image_source(url, overlay.base));
        let box_ = div()
            .relative()
            .rounded(px(Theme::button_radius()))
            .overflow_hidden()
            .border_1()
            .border_color(theme.border)
            .children(overlay.layouts.map(|layouts| {
                let layouts = layouts.clone();
                let ix = overlay.block;
                canvas(
                    move |bounds, _, _| layouts.record_picture(ix, bounds),
                    |_, _, _, _| (),
                )
                .absolute()
                .size_full()
            }));
        match width {
            // A stated width is the box's: it hugs, so the border is around
            // the picture rather than around the column beside it, and the
            // picture fills what the box settled on — which `max_w_full`
            // holds inside the page however wide the width was written.
            Some(width) => box_
                .self_start()
                .max_w_full()
                .w(px(width as f32))
                .child(picture.w(px(width as f32)).max_w_full()),
            // Unstated, the picture scales itself against the column, which
            // is a percentage and so needs a box that spans one to measure.
            None => box_.child(picture.max_w_full()),
        }
    };
    div()
        .flex()
        .flex_col()
        .gap(px(CAPTION_GAP))
        .child(picture)
        // An empty caption still paints while the caret is in it, or there
        // would be nothing to type into and no hint saying so.
        .when(
            overlay.caption == Caption::Shown && (!alt.is_empty() || overlay.caret().is_some()),
            |el| {
                el.child(text_element(
                    alt,
                    typography.caption.size(),
                    typography.caption.line_height(),
                    FontWeight::NORMAL,
                    overlay,
                    theme,
                    cx,
                ))
            },
        )
        .into_any_element()
}

/// A bookmark, in Notion's proportions: a fixed-height row with the text on the
/// left and an image panel of a fixed width on the right, all of it one click
/// target. [`Form::Embed`] turns the row into a column and gives the image the
/// card's full width instead, and [`Form::Chip`] is neither — a pill of favicon
/// and title, which is what an inline mention would be if shaped text had
/// anywhere to put a picture.
///
/// The row is a fixed height with its footer pinned to the bottom, because a
/// preview resolves *after* the card has painted — a blurb arriving into a box
/// that grows would shove every block below it down the page. An embed's cover
/// holds that height, so its text hugs.
fn bookmark(
    ix: usize,
    url: &str,
    form: Form,
    typography: &Typography,
    theme: &Theme,
    cx: &App,
) -> AnyElement {
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
            .rounded(px(Theme::control_radius()))
            .border_1()
            .border_color(theme.border)
            .bg(theme.element_hover)
            .text_size(px(typography.body.size()))
            .line_height(px(typography.body.line_height()))
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
        .px(px(CARD_PADDING))
        .py(px(CARD_PADDING - 2.0))
        .child(
            div()
                .truncate()
                .text_size(px(typography.body.size()))
                .line_height(px(typography.body.line_height()))
                .text_color(theme.text)
                .child(title),
        )
        .children(preview.description.map(|blurb| {
            div()
                .line_clamp(2)
                .text_size(px(typography.card.size()))
                .line_height(px(typography.card.line_height()))
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
                .text_size(px(typography.card.size()))
                .text_color(theme.text_muted)
                .child(mark(CARD_ICON))
                .child(div().truncate().child(label)),
        );

    let picture = corners(div(), form)
        .bg(theme.surface)
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .child(match preview.image {
            Some(image) => corners(img(image).size_full().object_fit(ObjectFit::Cover), form)
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
        .rounded(px(Theme::button_radius()))
        .border(px(CARD_BORDER))
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

/// The card's corners, on the panel that reaches them: a content mask is a
/// rectangle, so a picture paints square over a rounded card unless it carries
/// the radius itself, concentric inside the card's border.
fn corners<T: Styled>(element: T, form: Form) -> T {
    let corner = px(Theme::inset_radius(Theme::button_radius(), CARD_BORDER));
    match form {
        Form::Embed => element.rounded_t(corner),
        _ => element.rounded_r(corner),
    }
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
#[expect(
    clippy::too_many_arguments,
    reason = "a table, its overlay, and what paints them"
)]
fn table(
    align: &[Align],
    header: &[Text],
    rows: &[Vec<Text>],
    overlay: Overlay,
    typography: &Typography,
    theme: &Theme,
    window: &mut Window,
    cx: &App,
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
            let flat = flatten_with(cell, weight, theme, |name| {
                crate::marks::paint_of(cx, name, theme)
            });
            if !flat.text.is_empty() {
                let width = f32::from(
                    text_system
                        .shape_line(
                            flat.text.clone(),
                            px(typography.body.size()),
                            &flat.runs,
                            None,
                        )
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
                .text_size(px(typography.body.size()))
                .line_height(px(typography.body.line_height()));
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
                    typography.body.size(),
                    typography.body.line_height(),
                    overlay.at(Part::Cell { row, column: c }),
                    theme,
                ));
            }
            row_el = row_el.child(cell_el);
        }
        inner = inner.child(row_el);
    }

    ui::scroll::Viewport::new(
        format!("md-table-scroll-{ix}"),
        div()
            .id(ElementId::named_usize("md-table", ix))
            .w_full()
            .child(inner),
        gpui::Axis::Horizontal,
    )
    .into_any_element()
}
