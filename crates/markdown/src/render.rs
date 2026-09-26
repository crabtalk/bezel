//! [`Doc`] → gpui elements.
//!
//! Numbers drive layout (sizes, line heights, paddings — the constants here);
//! colors are paint, read from [`Theme`]. Blocks are a flat list, so nesting is
//! left padding rather than nested containers, and the gap between two blocks
//! is decided by the pair: list items sit tight, everything else breathes.
//!
//! Ported from zeronsh/comet (MIT) and rebuilt against the flat block model.

use std::{
    cell::RefCell,
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    ops::Range,
    path::Path,
    rc::Rc,
};

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
mod code;
mod layouts;
mod media;
mod table;
mod text;

pub use code::*;
pub use layouts::*;
use media::*;
use table::*;
pub use text::*;

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
        canvas(
            move |bounds, window, _| layouts.frame(bounds, window),
            |_, _, _, _| (),
        )
        .absolute()
        .size_full()
    });
    // Cloned once so the theme is readable while `cx` stays free for the
    // element state the copy button needs.
    let theme = Theme::of(cx).clone();
    let typography = typography.unwrap_or_else(|| Typography::of(cx));
    let highlight = crate::marks::highlight_paint_of(cx);
    let mut column = div().flex().flex_col().children(reset);

    // With layouts to measure into, a block outside the band stands in at its
    // last height. The band is last frame's, so it is dropped at the first
    // block never measured: nothing below it has a known position.
    let keys: Vec<u64> = match layouts {
        Some(layouts) => {
            let keys: Vec<u64> = doc
                .blocks
                .iter()
                .map(|block| block_key(block, &typography))
                .collect();
            layouts.prune(&keys);
            keys
        }
        None => Vec::new(),
    };
    let mut band = layouts.and_then(BlockLayouts::band);
    let caret = selection.map(|selection| (selection.head.block, selection.anchor.block));
    let mut top = px(0.0);
    let mut skipped: Option<(f32, Pixels)> = None;

    for (ix, block) in doc.blocks.iter().enumerate() {
        let gap = match doc.blocks.get(ix.wrapping_sub(1)) {
            None => 0.0,
            Some(previous) if tight(previous, block) => LIST_GAP,
            Some(_) => BLOCK_GAP,
        };
        if let (Some(layouts), Some(within)) = (layouts, band.clone()) {
            match layouts.height(keys[ix]) {
                Some(height) => {
                    let start = top + px(gap);
                    top = start + height;
                    let near = top >= within.start && start <= within.end;
                    let held = caret.is_some_and(|(head, anchor)| ix == head || ix == anchor);
                    if !near && !held {
                        skipped = Some(match skipped {
                            None => (gap, height),
                            Some((first, run)) => (first, run + px(gap) + height),
                        });
                        continue;
                    }
                }
                None => band = None,
            }
        }
        if let Some((first, run)) = skipped.take() {
            column = column.child(stand_in(first, run));
        }
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
            let key = keys[ix];
            canvas(
                move |bounds, _, _| {
                    layouts.record_block(ix, bounds);
                    layouts.record_height(key, bounds.size.height);
                },
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
    if let Some((first, run)) = skipped {
        column = column.child(stand_in(first, run));
    }

    column.into_any_element()
}

/// What a block's height is cached under: its content and the type it is set
/// in, so an edit elsewhere that shifts its index keeps the height.
fn block_key(block: &Block, typography: &Typography) -> u64 {
    let mut hasher = DefaultHasher::new();
    block.hash(&mut hasher);
    typography.body.size().to_bits().hash(&mut hasher);
    typography.body.line_height().to_bits().hash(&mut hasher);
    hasher.finish()
}

/// A run of blocks outside the band, as the space they took when last built.
/// Scrolled into view before a frame has built them — a jump further than the
/// band reaches — it asks for the frame that will.
fn stand_in(gap: f32, height: Pixels) -> gpui::Div {
    div().mt(px(gap)).h(height).w_full().relative().child(
        canvas(
            |bounds, window, _| {
                let shown = bounds.intersect(&window.content_mask().bounds);
                if shown.size.height > px(0.0) && shown.size.width > px(0.0) {
                    window.request_animation_frame();
                }
            },
            |_, _, _, _| (),
        )
        .absolute()
        .size_full(),
    )
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
