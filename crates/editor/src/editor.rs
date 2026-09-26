//! The editing surface.
//!
//! The document is the single source of truth. There is no `TextField` per
//! block: a selection is a pair of `Cursor`s into one `Doc`, which is what
//! makes Enter split, Backspace merge and Tab indent into list operations
//! rather than negotiations between separate widgets each owning a string.
//!
//! Everything about *what* an edit does lives in `markdown` — `edit` and
//! `select` — and is tested there without a window. This crate owns only what
//! needs one: a focus handle, key bindings, the platform input handler, and
//! turning a click into a position.

use gpui::{
    App, Context, CursorStyle, ElementInputHandler, EventEmitter, FocusHandle, Focusable,
    KeyContext, MouseButton, Render, Styled as _, Task, Window, canvas, div, prelude::*,
};
use markdown::{
    Annotation, Block, BlockKind, BlockLayouts, Cursor, Doc, Form, Mark, Part, Selection, Splice,
    Text, edit, edit::shortcut,
};
use motion::Painter;
use std::{ops::Range, time::Duration};
use theme::Theme;

use crate::{
    anchor::{Anchor, AnchorId, Delta},
    history::{EditKind, History},
    layout::Layout,
    link::{self, Choice},
    slash::Slash,
    text_size::{self, TextSize},
};

mod blocks;
mod clipboard;
pub(crate) mod image;
mod input;
pub mod keys;
pub(crate) mod menu;
mod mode;
mod pointer;
mod render;
mod typing;

pub use keys::init;
use keys::{
    Backspace, Copy, Cut, DecreaseTextSize, Delete, DeleteToHome, DeleteWordLeft, DeleteWordRight,
    Dismiss, DocumentEnd, DocumentStart, Down, DuplicateBlock, End, Home, IncreaseTextSize, Indent,
    InsertParagraph, KillLine, Left, MoveBlockDown, MoveBlockUp, Outdent, Paste, Redo, RemoveBlock,
    ResetTextSize, Right, SelectAll, SelectDocumentEnd, SelectDocumentStart, SelectDown, SelectEnd,
    SelectHome, SelectLeft, SelectRight, SelectUp, SelectWordLeft, SelectWordRight, SoftBreak,
    SplitBlock, ToggleBold, ToggleCode, ToggleHighlight, ToggleItalic, ToggleStrike, Undo, Up,
    WordLeft, WordRight,
};

pub const CONTEXT: &str = "BezelEditor";

/// The custom mark [`ToggleHighlight`] toggles. It does nothing until the app
/// registers a mark under this name with [`markdown::set_marks`].
pub const HIGHLIGHT_MARK: &str = "highlight";

/// [`CONTEXT`], which every binding in [`keys`] is scoped to, plus the mark
/// that keeps `tab` for [`Editor::indent`].
fn key_context() -> KeyContext {
    let mut context = KeyContext::default();
    context.add(CONTEXT);
    context.add(ui::focus::CLAIMS_TAB);
    context
}

/// What the editor tells its host about.
///
/// An app holding comment threads has to hear that the document moved, or its
/// side of the pairing goes stale against anchors that did not. Split the way
/// [`ui::input::FieldEvent`] is, so a listener takes only the half it needs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EditorEvent {
    /// The document is different, and the anchors have been mapped through it.
    Changed,
    /// A click landed on an anchor's range.
    AnchorActivated(AnchorId),
    /// The editor switched between the document and its source, which a host
    /// lighting its own toggle has no other way to hear about — the switch can
    /// come from an undo as well as from the button.
    ModeChanged(Mode),
}

/// Which form the document is being edited in.
///
/// The trigger is the app's: a button, a menu row, a chord of its own. What is
/// here is the switch it calls, because the caret, the undo history and the
/// focus have to survive it, and only the editor owns all three.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// The document as it reads.
    #[default]
    Blocks,
    /// The markdown a save would write, in one editable text.
    Source,
}

/// Which of the editor's own affordances are on.
///
/// All of them unless an app says otherwise: a document with nothing
/// discoverable on it is the wrong default for a library. Turning one off is
/// for an app that puts its own in the same place — a bar with its own block
/// menu does not want the gutter handle's as well — rather than for trimming.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chrome {
    /// The gutter handle, the menu it opens, and dragging a block by it.
    pub handle: bool,
    /// The `/` menu at an empty block.
    pub slash: bool,
    /// A fence's language label, and the picker it opens.
    pub language: bool,
    /// The menu a pasted URL drops — leave it, or make a card, a chip or the
    /// picture it points at.
    pub paste: bool,
}

impl Default for Chrome {
    fn default() -> Self {
        Self {
            handle: true,
            slash: true,
            language: true,
            paste: true,
        }
    }
}

/// What a toolbar reads to light itself, in one call.
///
/// Every field is a question a bar asks on every frame, and each was a separate
/// reach into the document before: which marks are lit, what the block is
/// called, whether cmd-E would fence, and whether any of it applies at all.
/// Taken together so a bar cannot answer half of them from one frame and half
/// from the next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Formatting {
    /// Which form the document is in. In [`Mode::Source`] the markup is already
    /// spelled out, so [`Self::marks`] is empty and a bar has nothing to light.
    pub mode: Mode,
    /// The marks the selection carries throughout — and at a collapsed caret,
    /// the ones the next character typed would carry, cmd-B before typing
    /// included.
    pub marks: Vec<Mark>,
    /// What the caret's block is called in [`turns`], or `None` for a block the
    /// menu does not offer.
    pub block: Option<gpui::SharedString>,
    /// Whether [`Mark::Code`] here makes a fence out of the selection rather
    /// than an inline span — the one chord whose meaning changes with what is
    /// selected, and the one a bar cannot work out for itself.
    pub fenceable: bool,
}

/// Every block a block can be turned into, and what each is called — the
/// vocabulary the slash menu and the block menu both offer, for an app building
/// a menu of its own. Pair with [`Editor::set_block`], which is what both of
/// bezel's own menus call.
pub fn turns() -> Vec<(gpui::SharedString, BlockKind)> {
    crate::slash::items()
}

/// Shown on the focused block while it is empty — the only discoverable place
/// to say that `/` does anything.
const PLACEHOLDER: &str = "Type / for commands";

/// What tab inserts in the source, where there are no blocks to indent. Two
/// spaces, which is what markdown's own nesting is written in.
const INDENT: &str = "  ";

/// Half the caret's blink period — `ui::TextField`'s, which is the 500ms on,
/// 500ms off macOS itself uses.
const BLINK: Duration = Duration::from_millis(500);

/// The handle's box. Wide enough to be hit without crowding the margin; how
/// far left of the text it sits is [`Layout::text_inset`](crate::Layout).
const HANDLE_SIZE: f32 = 18.0;

/// How far a drag on an image's edge handle can shrink it — matches
/// `markdown::render`'s `TABLE_MIN_COLUMN_WIDTH`, the same floor for the
/// other block content a drag can resize.
const MIN_IMAGE_WIDTH: f32 = 96.0;

/// Whether a selection is prose covering more than one line — two blocks, or
/// one line break inside a single block.
///
/// What a chord means about a selection is the editor's to decide; a [`Doc`]
/// has no opinion about it. A selection reaching into a table or a fence is not
/// prose and keeps the inline behaviour, because half a table has no lines to
/// make a fence out of.
fn fenceable(doc: &Doc, selection: Selection) -> bool {
    let spans = doc.spans(selection);
    let covered = |at: &Cursor, range: &Range<usize>| {
        doc.blocks[at.block]
            .text_at(at.part)
            .and_then(|text| text.text.get(range.clone()))
    };
    spans.iter().all(|(at, _)| at.part == Part::Body)
        && (spans.len() > 1
            || spans
                .iter()
                .any(|(at, range)| covered(at, range).is_some_and(|text| text.contains('\n'))))
}

/// Give an empty document a block to hold a caret, and say whether one was
/// needed.
///
/// A [`Doc`] with no blocks is a legitimate document — it is what `parse("")`
/// returns — but it is not something that can be edited: nothing paints, so
/// there is no caret and no placeholder, and neither a click nor a hit test
/// has a target to find. An empty file would sit inert until something typed
/// a block into existence, which is the one thing you cannot do with no caret.
fn ensure_block(doc: &mut Doc) -> bool {
    if !doc.blocks.is_empty() {
        return false;
    }
    doc.blocks
        .push(Block::new(BlockKind::Paragraph(Text::default())));
    true
}

fn line_home(at: Cursor, doc: &Doc) -> Cursor {
    let Some(text) = doc
        .blocks
        .get(at.block)
        .and_then(|block| block.text_at(at.part))
    else {
        return at.home();
    };
    Cursor {
        offset: ui::input::line_start(&text.text, at.offset.min(text.text.len())),
        ..at
    }
}

fn line_end(at: Cursor, doc: &Doc) -> Cursor {
    let Some(text) = doc
        .blocks
        .get(at.block)
        .and_then(|block| block.text_at(at.part))
    else {
        return at.end(doc);
    };
    Cursor {
        offset: ui::input::line_end(&text.text, at.offset.min(text.text.len())),
        ..at
    }
}

/// The document a source view is edited as: one fence holding the markdown.
///
/// A fence rather than a paragraph because a fence is the block whose caret
/// already behaves like a plain text editor's — Enter is a newline, nothing
/// typed into it is markup, and its lines are laid out one per source line.
fn source_doc(source: &str) -> Doc {
    Doc {
        blocks: vec![Block::new(BlockKind::Code {
            language: Some(markdown::source::LANGUAGES[0].to_string()),
            code: Text::plain(source),
        })],
    }
}

/// One of the two floating menus a block drops — the block it belongs to and
/// where it hangs. A `Popup` rather than an `Option` for the exit phase, and
/// for the press note: the card's `on_mouse_down_out` fires on the *press*, so
/// without one a trigger's click on the *release* reopens what it just shut.
pub(crate) type MenuPopup = ui::popover::Popup<(usize, gpui::Point<gpui::Pixels>)>;

pub struct Editor {
    doc: Doc,
    /// The dialect this document is read and written in — the app's own marks,
    /// taken once at construction. One editor, one spelling: a document that
    /// changed dialect between a read and a write would rewrite itself.
    marks: markdown::Marks,
    /// Which of the editor's own affordances paint. The app's, so one document
    /// can carry the lot and another none of it.
    chrome: Chrome,
    /// Which form the document is in. In [`Mode::Source`] `doc` is one fenced
    /// block holding the markdown, so every operation below that is about
    /// *blocks* asks [`Editor::blocks`] first.
    mode: Mode,
    /// Collapsed for an ordinary caret, so there is one position here rather
    /// than a caret and a range that can disagree.
    selection: Selection,
    focus_handle: FocusHandle,
    /// The IME composition range within the caret's text, underlined while it
    /// is being composed.
    marked: Option<Range<usize>>,
    /// Where each text landed last frame, so a click can be turned into a
    /// caret. Only paint knows this, so the renderer fills it.
    layouts: BlockLayouts,
    history: History,
    /// Which half of the blink the caret is in. Flipped by [`Self::start_blink`].
    caret_on: bool,
    /// The blink, alive only while the document holds focus.
    blink: Option<Task<()>>,
    /// Comment ranges, mapped through every edit and snapshotted with the
    /// document. Here rather than in the app because an undo restores a whole
    /// document and leaves no delta an app could map its own copy through.
    anchors: Vec<Anchor>,
    /// Marks the next typed character will carry — cmd-B at a collapsed caret,
    /// which otherwise has no range to apply to and so would do nothing.
    /// Cleared by any motion, because they belong to a spot and not to a mood.
    stored: Vec<Mark>,
    /// The open slash menu, if `/` started one.
    slash: Option<Slash>,
    /// The open paste menu, if a URL landed in a block of its own.
    pasted: Option<link::Paste>,
    /// The open prompt, if an image is waiting to be told where to look.
    url_prompt: Option<image::Prompt>,
    /// The block a file being dragged over the document would land after.
    dropping: Option<usize>,
    /// The block the pointer is over, which is the only one showing a handle.
    hovered: Option<usize>,
    /// A block being dragged by its handle, and where it would land.
    lifted: Option<(usize, usize)>,
    /// An image being dragged wider or narrower by its edge handle, and the
    /// width it holds now — `None` being the natural one, exactly as the
    /// document spells it. The document only learns the final width on
    /// release, the same reason `lifted` waits for the drop; carrying the
    /// document's own value here is what makes a press that never moved
    /// read back as no change at all.
    resizing: Option<(usize, Option<u32>)>,
    /// The block menu the handle opened, and where to anchor it.
    block_menu: MenuPopup,
    /// The language menu a fence's header opened, and the block it belongs to.
    language_menu: MenuPopup,
    /// Set by a floating layer's press — the gutter handle, the URL prompt —
    /// so the editor's own press does not undo what that press just did.
    press_claimed: bool,
    /// Where the editor's own box starts, so a position recorded in window
    /// coordinates can be placed inside it, and how far it reaches — which is
    /// how wide a resized image is allowed to be. Its own box rather than the
    /// picture's: a picture already narrowed would otherwise be its own
    /// ceiling, and no drag could ever widen it again.
    origin: gpui::Point<gpui::Pixels>,
    width: gpui::Pixels,
    /// Whether the pointer is dragging out a selection.
    dragging: bool,
    /// Whether the pointer is over painted text, which is the only place the
    /// editor claims an I-beam.
    over_text: bool,
    /// The host's scroll box, when it gave one, and whether the caret still
    /// owes it a reveal.
    scroll: Option<gpui::ScrollHandle>,
    reveal: bool,
    /// Where the gutter handle was placed this frame, so the frame after can
    /// tell whether the block moved out from under it.
    handle_at: Option<gpui::Point<gpui::Pixels>>,
    /// The column and row held across consecutive vertical moves.
    goal: Option<VerticalGoal>,
    /// The size the app set this document in, in points, or `None` to follow
    /// the app's own text size. Absolute rather than a factor over the ladder,
    /// so moving the interface size leaves a document set to 16pt at 16pt.
    ///
    /// What the chords move is the shared adjustment on top of this; the base
    /// itself is the app's alone.
    text_size: Option<f32>,
    /// The directory relative image paths resolve against.
    base: Option<std::path::PathBuf>,
}

#[derive(Clone, Copy)]
struct VerticalGoal {
    x: gpui::Pixels,
    /// Relative to the caret's painted position so scrolling cannot change the row.
    row_from_caret: gpui::Pixels,
}

impl Editor {
    pub fn new(source: &str, cx: &mut Context<Self>) -> Self {
        let marks = markdown::Marks::of(cx);
        let mut doc = markdown::parse_with(source, &marks);
        ensure_block(&mut doc);
        Self {
            marks,
            // Clamped, not defaulted: a document opening on a fence or a table
            // has no body at block zero, and a caret claiming one resolves
            // against nothing until something moves it.
            selection: Selection::at(Cursor::default().clamp(&doc)),
            doc,
            chrome: Chrome::default(),
            mode: Mode::default(),
            focus_handle: cx.focus_handle(),
            marked: None,
            layouts: BlockLayouts::default(),
            history: History::default(),
            caret_on: true,
            blink: None,
            anchors: Vec::new(),
            stored: Vec::new(),
            slash: None,
            pasted: None,
            url_prompt: None,
            dropping: None,
            hovered: None,
            lifted: None,
            resizing: None,
            block_menu: MenuPopup::default(),
            language_menu: MenuPopup::default(),
            press_claimed: false,
            origin: gpui::Point::default(),
            width: gpui::Pixels::ZERO,
            dragging: false,
            over_text: false,
            scroll: None,
            reveal: false,
            goal: None,
            handle_at: None,
            text_size: None,
            base: None,
        }
    }

    /// How many undo steps to keep. App-wide configuration would be a gpui
    /// global alongside [`init`], not a `Theme` field — the theme is rebuilt on
    /// every light/dark switch, which would quietly reset anything behavioural
    /// parked in it.
    pub fn with_undo_limit(mut self, limit: usize) -> Self {
        self.history = History::with_limit(limit);
        self
    }

    /// Read and write this document with marks of its own, rather than the ones
    /// [`markdown::set_marks`] installed. For an app whose editors do not all
    /// speak the same dialect.
    pub fn with_marks(mut self, marks: markdown::Marks) -> Self {
        let source = self.source();
        self.marks = marks;
        self.doc = markdown::parse_with(&source, &self.marks);
        ensure_block(&mut self.doc);
        self.selection = self.selection.clamp(&self.doc);
        self
    }

    /// Which of the editor's own affordances to paint. See [`Chrome`].
    pub fn with_chrome(mut self, chrome: Chrome) -> Self {
        self.chrome = chrome;
        self
    }

    /// What is painting now, for an app whose own bar mirrors it.
    pub fn chrome(&self) -> Chrome {
        self.chrome
    }

    /// Open in [`Mode::Source`] rather than on the document — an app whose
    /// editor is a markdown file first. Nothing is recorded: this is where the
    /// document starts, not a switch to step back over.
    pub fn with_mode(mut self, mode: Mode) -> Self {
        if mode != self.mode {
            self.switch(mode);
        }
        self
    }

    /// Open the document at a size of its own, in points — the app's settings
    /// field for prose. Unset, it is set at the app's own text size.
    ///
    /// The base, not the current size: `cmd-+` moves a shared adjustment over
    /// this, and `cmd-0` clears that adjustment to come back here.
    pub fn with_text_size(mut self, points: f32) -> Self {
        self.text_size = Some(points);
        self
    }

    /// Update the configured base size without changing the temporary zoom.
    pub fn set_text_size(&mut self, points: f32, cx: &mut Context<Self>) {
        if self.text_size != Some(points) {
            self.text_size = Some(points);
            cx.notify();
        }
    }

    /// The base the app set, if any. Add
    /// [`text_size_adjustment`](crate::text_size_adjustment) for what is on
    /// screen.
    pub fn text_size(&self) -> Option<f32> {
        self.text_size
    }

    /// Resolve relative image paths against `dir` — the document's own folder,
    /// for a document that keeps its pictures beside it. The stored URL stays
    /// as written.
    pub fn with_base(mut self, dir: impl Into<std::path::PathBuf>) -> Self {
        self.base = Some(dir.into());
        self
    }

    /// Change or clear the directory relative image paths resolve against.
    pub fn set_base(&mut self, dir: Option<std::path::PathBuf>, cx: &mut Context<Self>) {
        if self.base != dir {
            self.base = dir;
            cx.notify();
        }
    }

    /// The directory relative image paths resolve against, if one is set.
    pub fn base(&self) -> Option<&std::path::Path> {
        self.base.as_deref()
    }

    /// The box the document scrolls in, so typing off the bottom follows the
    /// caret down.
    ///
    /// The host's rather than the editor's: a document goes in whatever pane
    /// the app gives it, and the gutter handle, the drop indicator and the
    /// menus are all placed absolutely against this editor's own origin — put
    /// the scroll box here and every one of them would be offset twice.
    pub fn with_scroll(mut self, handle: gpui::ScrollHandle) -> Self {
        self.scroll = Some(handle);
        self
    }

    /// Bring the caret back into view.
    ///
    /// Read *after* paint, because a block that has only just appeared — the
    /// one Enter made — has no position recorded until it has painted once,
    /// which is exactly the case worth scrolling for.
    /// Ask for another frame when the block the handle sits on has moved.
    ///
    /// The handle is built from the records of the frame *before* this one —
    /// the document fills them as it paints, which is after the editor has
    /// finished building its tree — so a block that has just been indented, or
    /// grown a line, leaves the handle behind. Reading the records back here,
    /// once the document has painted, is what turns that into one late frame
    /// instead of a handle stranded until the caret blink happens to draw
    /// again.
    fn settle_handle(&mut self, window: &Window, cx: &mut Context<Self>) {
        let focused = self.focus_handle.is_focused(window);
        let now = self
            .handle_block(focused)
            .and_then(|ix| self.handle_origin(ix, cx));
        if now != self.handle_at {
            // Not `notify`: this runs *during* the draw, and the dirty flag it
            // sets is cleared when that draw finishes. Asking for the next
            // frame is what survives it.
            window.request_animation_frame();
        }
    }

    fn reveal_caret(&mut self, cx: &mut Context<Self>) {
        if !self.reveal {
            return;
        }
        let Some(scroll) = self.scroll.clone() else {
            self.reveal = false;
            return;
        };
        // Left set when the caret has not painted: a block with no text at all
        // never answers, and the next move is what gets it back.
        let Some((at, line)) = self.layouts.position(self.selection.head) else {
            return;
        };
        self.reveal = false;

        let view = scroll.bounds();
        let offset = scroll.offset();
        let mut y = offset.y;
        if at.y < view.top() {
            y += view.top() - at.y;
        } else if at.y + line > view.bottom() {
            y -= at.y + line - view.bottom();
        }
        // `set_offset` clamps nothing, and past the ends the document would
        // scroll away from the caret it was asked to show.
        let y = y.clamp(-scroll.max_offset().y, gpui::px(0.0));
        if y != offset.y {
            scroll.set_offset(gpui::point(offset.x, y));
            cx.notify();
        }
    }

    pub fn doc(&self) -> &Doc {
        &self.doc
    }

    pub fn selection(&self) -> Selection {
        self.selection
    }

    /// Put the selection somewhere — what a thread in a sidebar does when it is
    /// clicked, and what a caret restored with a document needs.
    ///
    /// Clamped, because the caller's range came from somewhere the document may
    /// have moved on from.
    pub fn select(&mut self, selection: Selection, cx: &mut Context<Self>) {
        self.selection = selection.clamp(&self.doc);
        self.history.interrupt();
        self.reveal = true;
        self.caret_moved();
        cx.notify();
    }

    /// Where the selection sits on screen, in window coordinates, so a host can
    /// float a toolbar at it.
    ///
    /// The head's row only — a selection spanning ten blocks wants its bubble
    /// where the pointer left off, not centred over the whole span. `None` when
    /// nothing is selected or the caret has not painted yet.
    /// Where everything landed last frame — blocks, pictures, a fence's
    /// language label, and the row rects of any range.
    ///
    /// Handed out whole rather than a method per question: an app placing
    /// chrome of its own asks the geometry, and which question it needs is not
    /// this crate's to guess. See [`markdown::BlockLayouts`].
    pub fn layouts(&self) -> &BlockLayouts {
        &self.layouts
    }

    pub fn selection_bounds(&self) -> Option<gpui::Bounds<gpui::Pixels>> {
        // The head alone. A bar centred over the whole selection wants
        // `layouts().rects(selection)`, which is every painted row of it.
        if self.selection.is_collapsed() {
            return None;
        }
        let (point, line_height) = self.layouts.position(self.selection.head)?;
        Some(gpui::Bounds::new(
            point,
            gpui::size(gpui::px(0.0), line_height),
        ))
    }

    /// The comment ranges, mapped up to date with the document.
    ///
    /// A range that reads [`Anchor::detached`] lost the words it pointed at.
    /// It is kept rather than dropped, because whether that means "outdated" or
    /// "resolved" is the app's question.
    pub fn anchors(&self) -> &[Anchor] {
        &self.anchors
    }

    /// Hand over the whole list — the app keeps the threads, this keeps their
    /// ranges. One entry point rather than add/remove/update, since the app is
    /// already holding the list that decides all three.
    pub fn set_anchors(&mut self, anchors: Vec<Anchor>, cx: &mut Context<Self>) {
        self.anchors = anchors;
        cx.notify();
    }

    /// The comment under a point in window coordinates — the space
    /// [`Self::anchor_bounds`] answers in and the press handler resolves in.
    ///
    /// The last match wins, so the newer of two overlapping ranges is the one a
    /// click opens.
    pub fn anchor_at(&self, at: gpui::Point<gpui::Pixels>) -> Option<AnchorId> {
        let at = self.layouts.hit(at)?;
        self.anchors
            .iter()
            .rfind(|anchor| {
                let (start, end) = anchor.range.ordered();
                !anchor.detached() && start <= at && at <= end
            })
            .map(|anchor| anchor.id)
    }

    /// Where to float a thread, mirroring [`Self::selection_bounds`].
    pub fn anchor_bounds(&self, id: AnchorId) -> Option<gpui::Bounds<gpui::Pixels>> {
        let anchor = self.anchors.iter().find(|anchor| anchor.id == id)?;
        let (point, line_height) = self.layouts.position(anchor.range.ordered().0)?;
        Some(gpui::Bounds::new(
            point,
            gpui::size(gpui::px(0.0), line_height),
        ))
    }

    /// The ranges the renderer washes, clamped because a block can change kind
    /// under an anchor and take its part with it.
    fn annotations(&self) -> Vec<(Selection, Annotation)> {
        self.anchors
            .iter()
            .filter(|anchor| !anchor.detached())
            .map(|anchor| (anchor.range.clamp(&self.doc), anchor.state))
            .collect()
    }

    /// A new caret position restarts its blink and ends any vertical run.
    /// Vertical motion records its next goal after moving the caret.
    fn caret_moved(&mut self) {
        self.blink = None;
        self.goal = None;
    }

    /// Blink the caret for as long as the document holds focus.
    fn start_blink(&mut self, cx: &mut Context<Self>) {
        self.caret_on = true;
        self.blink = Some(cx.spawn(async move |editor, cx| {
            loop {
                cx.background_executor().timer(BLINK).await;
                let flipped = editor.update(cx, |editor, cx| {
                    editor.caret_on = !editor.caret_on;
                    cx.notify();
                });
                if flipped.is_err() {
                    break;
                }
            }
        }));
    }

    /// Where typing would land — the moving end of the selection.
    fn cursor(&self) -> Cursor {
        self.selection.head
    }

    /// Put the caret somewhere, collapsed.
    fn place(&mut self, cursor: Cursor) {
        self.selection = Selection::at(cursor.clamp(&self.doc));
    }

    /// Move the head, extending the selection or collapsing it — the one path
    /// every motion key takes, so shift is a flag rather than a second handler.
    fn moved(
        &mut self,
        extend: bool,
        to: impl FnOnce(Cursor, &Doc) -> Cursor,
        cx: &mut Context<Self>,
    ) {
        let head = to(self.selection.head, &self.doc).clamp(&self.doc);
        self.head_to(head, extend);
        cx.notify();
    }

    /// Delete from the caret to wherever `to` lands — every kill chord, sharing
    /// the cursor functions the motion chords use so the two cannot disagree.
    ///
    /// Nothing left to take within the block — the target crossed out of it, or
    /// landed on the caret — is the block edge, and `forward` is which edge:
    /// [`Self::delete_forward`] joins the next block, [`Self::delete_back`]
    /// outdents or strips block syntax before it merges anything. The direction
    /// has to be the chord's own, because a target that lands on the caret is
    /// the same cursor whichever way it was reaching.
    fn delete_to(
        &mut self,
        forward: bool,
        to: impl FnOnce(Cursor, &Doc) -> Cursor,
        cx: &mut Context<Self>,
    ) {
        if !self.selection.is_collapsed() {
            return self.delete_back(cx);
        }
        let at = self.cursor();
        let target = to(at, &self.doc).clamp(&self.doc);
        if target.block != at.block || target.part != at.part || target.offset == at.offset {
            return if forward {
                self.delete_forward(cx)
            } else {
                self.delete_back(cx)
            };
        }
        let painter = Painter::of(cx);
        self.edit(EditKind::Delete, cx, |this| {
            let splice = this
                .doc
                .replace(Selection::new(target, at), Text::default());
            this.selection = Selection::at(splice.caret.clamp(&this.doc));
            this.track_slash("", painter);
            vec![Delta::Spliced(splice)]
        });
    }

    fn head_to(&mut self, head: Cursor, extend: bool) {
        self.selection = if extend {
            self.selection.extend_to(head)
        } else {
            Selection::at(head)
        };
        // A motion ends the undo group: typing a word, moving away and typing
        // again must not undo as one step across two places. It also spends any
        // stored mark and any open paste menu, both of which belonged to the
        // spot the caret just left.
        self.history.interrupt();
        self.stored.clear();
        self.pasted = None;
        self.reveal = true;
        self.caret_moved();
    }

    /// Every mutation goes through here, so none of them can forget to record
    /// a step and none of them has to know how steps coalesce.
    fn edit(
        &mut self,
        kind: EditKind,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut Self) -> Vec<Delta>,
    ) {
        // Any edit answers the paste menu by ignoring it — whatever it offered
        // was about a block that no longer holds only the link.
        self.pasted = None;
        self.history
            .record(kind, self.mode, &self.doc, self.selection, &self.anchors);
        // A list rather than one: Enter clears a selection *and* splits, and an
        // anchor mapped through only half of that lands in the wrong place.
        // Source mode maps nothing: its deltas are about one fence, and an
        // anchor dragged through those would point at the markup. They are
        // clamped back onto the document on the way out instead.
        for delta in edit(self) {
            if !self.blocks() {
                continue;
            }
            for anchor in &mut self.anchors {
                anchor.map(&delta);
            }
        }
        // Deleting the last block is the other way to an empty document, and
        // the caret belongs at the start of whatever replaces it.
        if ensure_block(&mut self.doc) {
            self.selection = Selection::at(Cursor::default());
        }
        if !self.blocks() {
            self.ensure_source();
        }
        self.history.landed(kind, self.selection);
        // Typing moves the caret as surely as an arrow key does, and a split
        // moves it onto a block that does not exist until this frame paints.
        self.reveal = true;
        self.caret_moved();
        cx.emit(EditorEvent::Changed);
        cx.notify();
    }

    /// Up and down, by one painted row.
    ///
    /// Geometry rather than arithmetic on line numbers, so a wrapped paragraph,
    /// a code block's lines and a table's rows are all the same case and none
    /// needs counting — but geometry walked in document order rather than
    /// hit-tested, which is [`markdown::BlockLayouts::step_row`]'s whole point.
    /// Falls back to the block-wise motion off either end of the document, and
    /// on the first frame, when nothing has painted to walk.
    fn vertical(&mut self, down: bool, extend: bool, cx: &mut Context<Self>) {
        // Up and down walk a menu while it is open, not the document.
        let delta = if down { 1 } else { -1 };
        if let Some(pasted) = &mut self.pasted {
            pasted.step(delta);
            return cx.notify();
        }
        if let Some(slash) = &mut self.slash {
            slash.step(delta);
            return cx.notify();
        }
        let head = self.selection.head;
        let Some((at, _)) = self.layouts.position(head) else {
            return self.moved(
                extend,
                |at, doc| if down { at.down(doc) } else { at.up(doc) },
                cx,
            );
        };
        let from = self
            .goal
            .map_or(at, |goal| gpui::point(goal.x, at.y + goal.row_from_caret));
        match self.layouts.step_row(head, from, down) {
            Some((to, row)) => {
                self.head_to(to.clamp(&self.doc), extend);
                self.goal = self
                    .layouts
                    .position(self.cursor())
                    .map(|(caret, _)| VerticalGoal {
                        x: from.x,
                        row_from_caret: row - caret.y,
                    });
            }
            // Off the top is the start of the document and off the bottom is
            // its end, which is what every native field does.
            None => {
                // Except where the end is a block a caret cannot carry on from,
                // and going down means the paragraph after it — the one a click
                // below the document asks for by the same rule.
                if down
                    && !extend
                    && self.cursor().block + 1 == self.doc.blocks.len()
                    && self.append_tail(cx)
                {
                    return;
                }
                let to = if down {
                    head.down(&self.doc)
                } else {
                    head.up(&self.doc)
                };
                self.head_to(to.clamp(&self.doc), extend);
                // The column outlives the trip to either end, so coming back
                // retraces the path.
                self.goal = Some(VerticalGoal {
                    x: from.x,
                    row_from_caret: gpui::Pixels::ZERO,
                });
            }
        }
        cx.notify();
    }

    /// Shift-tab in the source: take back up to one [`INDENT`] of the spaces
    /// before the caret, and nothing else — a line that is not indented has
    /// nothing to give.
    fn unindent(&mut self, cx: &mut Context<Self>) {
        let at = self.cursor();
        let Some(text) = self.caret_text() else {
            return;
        };
        let before = &text.text[..at.offset];
        let width = before.len() - before.trim_end_matches(' ').len();
        let width = width.min(INDENT.len());
        if width == 0 {
            return;
        }
        self.edit(EditKind::Delete, cx, |this| {
            let from = Cursor::new(at.block, at.part, at.offset - width);
            let splice = this.doc.replace(Selection::new(from, at), Text::default());
            this.selection = Selection::at(splice.caret.clamp(&this.doc));
            vec![Delta::Spliced(splice)]
        });
    }

    /// The caret's text, for the input handler's offset arithmetic.
    fn caret_text(&self) -> Option<&Text> {
        let at = self.cursor();
        self.doc.blocks.get(at.block)?.text_at(at.part)
    }
}

impl EventEmitter<EditorEvent> for Editor {}

impl Focusable for Editor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
