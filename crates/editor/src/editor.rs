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

pub(crate) mod image;
mod input;
pub mod keys;
pub(crate) mod menu;

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

    /// The document as markdown — normalized, because that is the form that
    /// survives being read back. In [`Mode::Source`] it is the text being
    /// edited, exactly as it stands.
    pub fn source(&self) -> String {
        match self.mode {
            Mode::Blocks => {
                let mut doc = self.doc.clone();
                doc.normalize_with(&self.marks);
                markdown::serialize_with(&doc, &self.marks)
            }
            Mode::Source => self.source_text().to_string(),
        }
    }

    /// Which form the document is being edited in.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// What a toolbar needs to light itself. See [`Formatting`].
    pub fn formatting(&self) -> Formatting {
        let at = self.cursor();
        let marks = if self.blocks() {
            let mut marks = self.doc.marks(self.selection);
            // A stored mark is one cmd-B has already taken and nothing has
            // spent yet, so the button that took it stays lit.
            for mark in &self.stored {
                if !marks.contains(mark) {
                    marks.push(mark.clone());
                }
            }
            marks
        } else {
            Vec::new()
        };
        Formatting {
            mode: self.mode,
            marks,
            block: self
                .doc
                .blocks
                .get(at.block)
                .and_then(|block| crate::slash::label(&block.kind)),
            fenceable: self.blocks() && fenceable(&self.doc, self.selection),
        }
    }

    /// Switch between the document and its markdown, carrying the caret across.
    ///
    /// One undo step, and one the history knows the mode of: stepping back
    /// over a switch puts the document back in the form it was edited in.
    ///
    /// A comment anchor does not follow an edit made to the source — there are
    /// no blocks there to anchor to — and is clamped back onto the document on
    /// the way out.
    pub fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        if mode == self.mode {
            return;
        }
        self.history.record(
            EditKind::Structure,
            self.mode,
            &self.doc,
            self.selection,
            &self.anchors,
        );
        self.dismiss_menus();
        self.switch(mode);
        self.history.landed(EditKind::Structure, self.selection);
        self.reveal = true;
        self.caret_moved();
        cx.emit(EditorEvent::ModeChanged(mode));
        cx.emit(EditorEvent::Changed);
        cx.notify();
    }

    /// Turn the document into the other form, caret and all. The half of
    /// [`Self::set_mode`] that [`Self::with_mode`] needs without a window.
    fn switch(&mut self, mode: Mode) {
        match mode {
            Mode::Source => {
                let (source, offset) =
                    markdown::serialize_at(&self.doc, self.cursor(), &self.marks);
                self.doc = source_doc(&source);
                self.selection = Selection::at(Cursor::new(0, Part::Code, offset));
            }
            Mode::Blocks => {
                let (doc, at) =
                    markdown::parse_at(self.source_text(), self.cursor().offset, &self.marks);
                self.doc = doc;
                ensure_block(&mut self.doc);
                self.selection = Selection::at(at.clamp(&self.doc));
                for anchor in &mut self.anchors {
                    anchor.range = anchor.range.clamp(&self.doc);
                }
            }
        }
        self.mode = mode;
    }

    /// [`Mode::Source`] if the document is in blocks, and back again — what a
    /// toggle in the app's own chrome calls.
    pub fn toggle_source(&mut self, cx: &mut Context<Self>) {
        self.set_mode(
            match self.mode {
                Mode::Blocks => Mode::Source,
                Mode::Source => Mode::Blocks,
            },
            cx,
        );
    }

    /// Whether the document is the thing being edited, rather than its source.
    ///
    /// Every operation that acts on *blocks* asks this: in source mode there is
    /// one block, it is a fence holding a string, and turning it into a heading
    /// or dragging it somewhere would edit the markup rather than the document
    /// the markup spells.
    fn blocks(&self) -> bool {
        self.mode == Mode::Blocks
    }

    /// The text of the fence the source is held in — what is being edited in
    /// [`Mode::Source`]. Only meaningful there; in [`Mode::Blocks`] the
    /// document is the truth and this is whatever block zero happens to be.
    fn source_text(&self) -> &str {
        self.doc
            .blocks
            .first()
            .and_then(|block| block.text_at(Part::Code))
            .map_or("", |text| text.text.as_str())
    }

    /// Put the source back in the one fence it is edited as.
    ///
    /// Backspace at the start of an empty fence is a merge, and a merge with
    /// nothing above it leaves a paragraph — a block the source view does not
    /// paint and the caret would be stranded in. The text survives either way,
    /// so this is a change of container and never of content.
    fn ensure_source(&mut self) {
        if self.doc.blocks.len() == 1 && matches!(self.doc.blocks[0].kind, BlockKind::Code { .. }) {
            return;
        }
        let source = self
            .doc
            .blocks
            .iter()
            .filter_map(|block| block.text_at(*block.parts().first()?))
            .map(|text| text.text.clone())
            .collect::<Vec<_>>()
            .join("\n");
        let offset = self.selection.head.offset.min(source.len());
        self.doc = source_doc(&source);
        self.selection = Selection::at(Cursor::new(0, Part::Code, offset));
    }

    /// Shut everything floating. A switch of mode is a new document as far as
    /// a menu anchored to a block is concerned.
    fn dismiss_menus(&mut self) {
        self.slash = None;
        self.pasted = None;
        self.url_prompt = None;
        self.hovered = None;
        self.lifted = None;
        self.dropping = None;
    }

    /// Replace whatever is selected with `text`, applying a markdown prefix if
    /// one completes.
    ///
    /// Typing, backspace, delete and IME all land here, so none of them has to
    /// ask whether a selection was empty.
    fn insert(&mut self, text: &str, cx: &mut Context<Self>) {
        let painter = Painter::of(cx);
        self.edit(EditKind::Insert, cx, |this| {
            let mut typed = Text::plain(text);
            // A stored mark applies to what is typed next and to nothing else,
            // so it is spent here.
            for mark in this.stored.drain(..) {
                typed.marks.push(markdown::MarkSpan {
                    range: 0..typed.text.len(),
                    mark,
                });
            }
            let splice = this.doc.replace(this.selection, typed);
            this.selection = Selection::at(splice.caret.clamp(&this.doc));
            let shortcut = this.apply_shortcut();
            let promoted = this.promote_quote_marker();
            let inline = this.apply_inline_rule();
            this.track_slash(text, painter);
            std::iter::once(Delta::Spliced(splice))
                .chain(shortcut)
                .chain(promoted)
                .chain(inline)
                .collect()
        });
    }

    /// Open the menu on a typed `/`, and keep its query in step afterwards.
    ///
    /// The query is the text between the `/` and the caret, so there is no
    /// second field and no focus to hand over — typing filters because typing
    /// is what it already was.
    fn track_slash(&mut self, typed: &str, painter: Painter) {
        if !self.chrome.slash {
            return;
        }
        let at = self.cursor();
        let text = self
            .doc
            .blocks
            .get(at.block)
            .and_then(|block| block.text_at(at.part))
            .map(|text| text.text.clone())
            .unwrap_or_default();

        if self.slash.is_none() {
            // Only a `/` that starts a word — a URL's slashes are not commands.
            let opened = at.offset.checked_sub(1).filter(|_| typed == "/");
            let starts_word = opened.is_none_or(|slash| {
                text[..slash]
                    .chars()
                    .next_back()
                    .is_none_or(char::is_whitespace)
            });
            // Only in a body: a fence holds its slash literally, and a caption
            // belongs to a block that is already what it is.
            if let Some(slash) = opened.filter(|_| starts_word && at.part == Part::Body) {
                self.slash = Some(Slash::open(
                    Cursor {
                        offset: slash,
                        ..at
                    },
                    painter,
                ));
            }
            return;
        }

        // Anything that leaves the run — a space, a click away, backspacing
        // onto the slash — closes it.
        let Some(query) = self.slash.as_ref().and_then(|slash| slash.query(at, &text)) else {
            self.slash = None;
            return;
        };
        if let Some(slash) = &mut self.slash {
            slash.refilter(&query);
        }
    }

    /// Take the highlighted block, replacing the `/query` that summoned it.
    /// Take `kind`, or the highlighted row when the caller names none — Enter
    /// and a click are the same operation with a different source.
    pub(super) fn confirm_slash(
        &mut self,
        kind: Option<BlockKind>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(slash) = &self.slash else {
            return false;
        };
        let (at, kind) = (slash.at, kind.or_else(|| slash.choice()));
        self.slash = None;
        let Some(kind) = kind else {
            return false;
        };
        let caret = self.cursor();
        self.edit(EditKind::Structure, cx, |this| {
            this.doc
                .edit_at(at, |text| text.remove(at.offset..caret.offset));
            this.doc.set_kind(at.block, kind);
            this.selection =
                Selection::at(Cursor::new(at.block, Part::Body, at.offset).clamp(&this.doc));
            vec![Delta::Spliced(Splice {
                removed: Selection::new(at, caret),
                caret: at,
                blocks: 0,
            })]
        });
        true
    }

    /// Collapse `**bold**` into a mark when its closing delimiter is typed.
    ///
    /// Runs after the insertion, on the text as it now stands, so a paste and a
    /// keystroke reach it the same way.
    fn apply_inline_rule(&mut self) -> Vec<Delta> {
        let at = self.cursor();
        // Code is literal to its closing fence, and a caption holds no mark a
        // `![...]` could spell.
        if matches!(at.part, Part::Code | Part::Caption) {
            return Vec::new();
        }
        let Some(text) = self
            .doc
            .blocks
            .get(at.block)
            .and_then(|block| block.text_at(at.part))
        else {
            return Vec::new();
        };
        let Some((open, inner, mark)) = edit::inline_rule(&text.text, at.offset) else {
            return Vec::new();
        };
        let width = open.len();
        let (opening, closing) = (open.clone(), inner.end..at.offset);
        self.doc.edit_at(at, |text| {
            // The closing delimiter first — taking the opening one would move
            // every offset after it.
            text.remove(inner.end..at.offset);
            text.remove(open);
            text.toggle(inner.start - width..inner.end - width, mark);
        });
        self.selection =
            Selection::at(Cursor::new(at.block, at.part, at.offset - 2 * width).clamp(&self.doc));
        // In the order the two removals went. The opening delimiter is ahead of
        // the closing one, so taking that one first left its offsets standing.
        vec![Self::taken(at, closing), Self::taken(at, opening)]
    }

    /// Add `mark` over the selection, or take it away if the whole selection
    /// already carries it. Public because a toolbar reaches the same operation
    /// the key does.
    pub fn toggle_mark(&mut self, mark: Mark, cx: &mut Context<Self>) {
        // Nothing in the source is a mark: the markup is already spelled out,
        // and cmd-B over `**bold**` would fence what it reads.
        if !self.blocks() {
            return;
        }
        // A caret inside a fence is enough to leave one, so this is the mark
        // that does not wait for a range: nothing typed into code is markup,
        // which leaves a stored mark there nothing to mean.
        let leaving_code = matches!(mark, Mark::Code) && self.cursor().part == Part::Code;
        // With nothing selected there is no range to mark, so the mark waits
        // for the next character — ProseMirror's stored marks, and the only way
        // cmd-B before typing can mean anything.
        if self.selection.is_collapsed() && !leaving_code {
            match self.stored.iter().position(|stored| *stored == mark) {
                Some(ix) => drop(self.stored.remove(ix)),
                None => self.stored.push(mark),
            }
            return cx.notify();
        }
        let selection = self.selection;
        self.edit(EditKind::Structure, cx, |this| {
            // Code over more than one line is a fence, which is the only shape
            // markdown has for it, and the same key is the way back out.
            let before = this.doc.blocks.len();
            // Fencing and unfencing rebuild the blocks the selection covered,
            // so what sat inside it has no position left to keep.
            let refenced = |doc: &Doc, head: Cursor| {
                vec![Delta::Spliced(Splice {
                    removed: selection,
                    caret: head,
                    blocks: doc.blocks.len() as isize - before as isize,
                })]
            };
            if matches!(mark, Mark::Code) {
                if let Some(head) = this.doc.unfence(selection) {
                    this.selection = Selection::at(head.clamp(&this.doc));
                    return refenced(&this.doc, head);
                }
                if fenceable(&this.doc, selection) {
                    let head = this.doc.fence(selection);
                    this.selection = Selection::at(head.clamp(&this.doc));
                    return refenced(&this.doc, head);
                }
            }
            // A mark is paint over text that does not move.
            this.doc.toggle_mark(selection, mark);
            vec![]
        });
    }

    /// Turn a typed prefix into the block it spells — `## ` into a heading.
    ///
    /// Runs after every insertion rather than only on space, because the
    /// vocabulary includes prefixes that end in one (`- [ ] `) and prefixes
    /// that do not (```` ``` ````).
    fn apply_shortcut(&mut self) -> Option<Delta> {
        let at = self.cursor();
        // A prefix is block syntax; inside a code fence or a table cell it is
        // the literal text the author typed.
        if at.part != Part::Body {
            return None;
        }
        let text = self.doc.blocks.get(at.block)?.text_at(Part::Body)?;
        // Only from the very start of a block, and only up to the caret: a
        // `- ` typed in the middle of a sentence is a hyphen.
        let (hit, len) = shortcut(&text.text)?;
        if at.offset < len {
            return None;
        }
        // Strip the prefix, then turn the block — the same two steps the slash
        // menu takes, so a `## ` and a menu pick land in one place.
        self.doc.edit_at(at, |text| text.remove(0..len));
        self.doc.set_kind(at.block, hit.apply(Text::default()));
        self.selection =
            Selection::at(Cursor::new(at.block, Part::Body, at.offset - len).clamp(&self.doc));
        // The transformation is its own step: undo after typing `## Title`
        // should give back the heading, not the paragraph before the hashes.
        self.history.interrupt();
        Some(Self::taken(at, 0..len))
    }

    /// Promote a quote whose first line is a GFM alert marker into the alert
    /// block kind the parser would have produced from the same markdown.
    ///
    /// The marker line goes the way a block shortcut's prefix does, so what was
    /// written under it stays as the alert's body.
    fn promote_quote_marker(&mut self) -> Option<Delta> {
        let at = self.cursor();
        if at.part != Part::Body {
            return None;
        }
        let block = self.doc.blocks.get(at.block)?;
        if !matches!(block.kind, BlockKind::Quote { kind: None, .. }) {
            return None;
        }
        let text = &block.text_at(Part::Body)?.text;
        let first = text.split('\n').next().unwrap_or_default();
        let kind = Self::quote_marker(first)?;
        // The marker line, and the break after it where a body follows.
        let len = first.len() + usize::from(text.len() > first.len());
        self.doc.edit_at(at, |text| text.remove(0..len));
        self.doc.set_kind(
            at.block,
            BlockKind::Quote {
                kind: Some(kind),
                text: Text::default(),
            },
        );
        self.selection = Selection::at(
            Cursor::new(at.block, Part::Body, at.offset.saturating_sub(len)).clamp(&self.doc),
        );
        self.history.interrupt();
        Some(Self::taken(at, 0..len))
    }

    /// The delta for `range` taken out of the text `at` is in — what an anchor
    /// sitting in that text has to move through.
    fn taken(at: Cursor, range: Range<usize>) -> Delta {
        let start = Cursor::new(at.block, at.part, range.start);
        Delta::Spliced(Splice {
            removed: Selection {
                anchor: start,
                head: Cursor::new(at.block, at.part, range.end),
            },
            caret: start,
            blocks: 0,
        })
    }

    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        self.delete_back(cx);
    }

    fn quote_marker(text: &str) -> Option<markdown::QuoteKind> {
        [
            markdown::QuoteKind::Note,
            markdown::QuoteKind::Tip,
            markdown::QuoteKind::Important,
            markdown::QuoteKind::Warning,
            markdown::QuoteKind::Caution,
        ]
        .into_iter()
        .find(|kind| text.eq_ignore_ascii_case(kind.marker()))
    }

    /// Delete backwards: the selection if there is one, otherwise the character
    /// before the caret, otherwise whatever the start of a block means.
    ///
    /// The kill chords land here too when they have nothing left to take within
    /// the block, so reaching out of one is decided in a single place.
    fn delete_back(&mut self, cx: &mut Context<Self>) {
        let at = self.cursor();
        // Reaching out of a block is structural; taking a character is not.
        let kind = if self.selection.is_collapsed() && at.offset == 0 {
            EditKind::Structure
        } else {
            EditKind::Delete
        };
        let painter = Painter::of(cx);
        self.edit(kind, cx, |this| {
            let before = this.doc.blocks.len();
            let splice = if !this.selection.is_collapsed() {
                this.doc.replace(this.selection, Text::default())
            } else if at.offset > 0 {
                this.doc
                    .replace(Selection::new(at.left(&this.doc), at), Text::default())
            } else {
                // `merge_back` outdents, unmarkers, unfences or merges —
                // whichever the block's state calls for — and says where the
                // caret landed.
                match this.doc.merge_back(at) {
                    // The seam between where the caret was and where it landed
                    // is exactly what the merge closed up.
                    Some(head) => Splice {
                        removed: Selection::new(head, at),
                        caret: head,
                        blocks: this.doc.blocks.len() as isize - before as isize,
                    },
                    None => return vec![],
                }
            };
            let head = splice.caret;
            this.selection = Selection::at(head.clamp(&this.doc));
            // Deleting narrows the query too, and backspacing onto the slash
            // itself is what closes the menu.
            this.track_slash("", painter);
            vec![Delta::Spliced(splice)]
        });
    }

    fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        self.delete_forward(cx);
    }

    /// Delete forwards, joining the next block when the caret is at the end of
    /// this one — which is what a kill to the end of a line does there too.
    fn delete_forward(&mut self, cx: &mut Context<Self>) {
        self.edit(EditKind::Delete, cx, |this| {
            let at = this.cursor();
            let range = if this.selection.is_collapsed() {
                Selection::new(at, at.right(&this.doc))
            } else {
                this.selection
            };
            let splice = this.doc.replace(range, Text::default());
            this.selection = Selection::at(splice.caret.clamp(&this.doc));
            vec![Delta::Spliced(splice)]
        });
    }

    /// Whether an open menu answered Enter itself.
    ///
    /// Every Enter chord asks first, or picking a block would also edit the one
    /// it is turning — and a chord the menu never sees leaves it open over a
    /// query the caret has walked away from.
    fn menu_took_enter(&mut self, cx: &mut Context<Self>) -> bool {
        if let Some(choice) = self.pasted.as_ref().map(link::Paste::choice) {
            self.confirm_paste(choice, cx);
            return true;
        }
        self.confirm_slash(None, cx)
    }

    /// Enter. In a body it splits the block; in a code fence it is a newline,
    /// which is the whole reason a fence is worth typing into.
    fn split_block(&mut self, _: &SplitBlock, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu_took_enter(cx) {
            return;
        }
        let at = self.cursor();
        match at.part {
            Part::Code => return self.insert("\n", cx),
            // A cell is one line by definition; Enter has nowhere to put a
            // break, so it does nothing rather than something surprising.
            Part::Cell { .. } => return,
            // An image with nothing to show yet is missing one thing, so Enter
            // asks for it rather than carrying on past a blank.
            Part::Caption
                if matches!(
                    self.doc.blocks.get(at.block).map(|block| &block.kind),
                    Some(BlockKind::Image { url, .. }) if url.is_empty()
                ) =>
            {
                return self.prompt_for_url(at.block, window, cx);
            }
            // A caption is one line too, but the block it belongs to is the
            // end of something — so Enter carries on underneath the picture,
            // which is [`Doc::split`]'s answer for a block with no body.
            Part::Caption | Part::Body => {}
        }
        self.edit(EditKind::Structure, cx, |this| {
            let mut deltas = Vec::new();
            if !this.selection.is_collapsed() {
                let splice = this.doc.replace(this.selection, Text::default());
                this.selection = Selection::at(splice.caret.clamp(&this.doc));
                deltas.push(Delta::Spliced(splice));
            }
            let at = this.cursor();
            let new = this.doc.split(at.block, at.offset);
            this.selection = Selection::at(Cursor::new(new, Part::Body, 0).clamp(&this.doc));
            // What followed the caret moved into a block of its own, which
            // everything below it now sits under.
            deltas.push(Delta::Spliced(Splice {
                removed: Selection::at(at),
                caret: Cursor::new(new, Part::Body, 0),
                blocks: 1,
            }));
            deltas
        });
    }

    /// Shift+Enter. In prose it keeps the caret in the block and inserts a
    /// literal newline; the places markdown itself keeps to one line still do.
    fn soft_break(&mut self, _: &SoftBreak, _: &mut Window, cx: &mut Context<Self>) {
        if self.menu_took_enter(cx) {
            return;
        }
        match self.cursor().part {
            Part::Body | Part::Code => self.insert("\n", cx),
            Part::Caption | Part::Cell { .. } => {}
        }
    }

    /// Ctrl+Enter. Open a plain paragraph after the current block's whole
    /// subtree and leave the current block's text untouched.
    fn insert_paragraph(&mut self, _: &InsertParagraph, _: &mut Window, cx: &mut Context<Self>) {
        if self.menu_took_enter(cx) {
            return;
        }
        if !self.blocks() {
            return;
        }
        self.edit(EditKind::Structure, cx, |this| {
            let at = this.cursor().block;
            let insert_at = this.doc.subtree(at).end;
            let indent = this.doc.blocks.get(at).map_or(0, |block| block.indent);
            this.doc.blocks.insert(
                insert_at,
                Block::at(BlockKind::Paragraph(Text::default()), indent),
            );
            this.doc.repair();
            this.selection = Selection::at(Cursor::new(insert_at, Part::Body, 0).clamp(&this.doc));
            vec![Delta::Opened {
                at: insert_at,
                count: 1,
            }]
        });
    }

    fn indent(&mut self, _: &Indent, _: &mut Window, cx: &mut Context<Self>) {
        // In the source there is one block to indent and indenting it would be
        // invisible, so tab is what it is in any text editor: two spaces.
        if !self.blocks() {
            return self.insert(INDENT, cx);
        }
        self.edit(EditKind::Structure, cx, |this| {
            this.doc.indent(this.cursor().block);
            vec![]
        });
    }

    fn outdent(&mut self, _: &Outdent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.blocks() {
            return self.unindent(cx);
        }
        self.edit(EditKind::Structure, cx, |this| {
            this.doc.outdent(this.cursor().block);
            vec![]
        });
    }

    fn increase_text_size(&mut self, _: &IncreaseTextSize, _: &mut Window, cx: &mut Context<Self>) {
        self.step_text_size(TextSize::of(cx).step, cx);
    }

    fn decrease_text_size(&mut self, _: &DecreaseTextSize, _: &mut Window, cx: &mut Context<Self>) {
        self.step_text_size(-TextSize::of(cx).step, cx);
    }

    fn reset_text_size(&mut self, _: &ResetTextSize, _: &mut Window, cx: &mut Context<Self>) {
        text_size::reset_text_size(cx);
    }

    /// Sizing is not an edit: it changes nothing about the document, so it
    /// leaves no undo step and no anchor moves.
    ///
    /// The step is taken against *this* document's size and stored back as the
    /// shared adjustment, so a press at the end of the range banks up nothing
    /// to work back through on the way down.
    fn step_text_size(&mut self, by: f32, cx: &mut Context<Self>) {
        let base = self.text_size.unwrap_or_else(theme::base_text_size);
        let next = TextSize::of(cx).clamp(text_size::resolve(self.text_size, cx) + by);
        text_size::set_adjustment(next - base, cx);
    }

    /// Escape closes an open menu, and otherwise collapses a selection — the
    /// things there are to back out of, innermost first.
    fn dismiss(&mut self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        if self.pasted.take().is_none() && self.slash.take().is_none() {
            self.selection = Selection::at(self.selection.head);
        }
        cx.notify();
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.selection = Selection::all(&self.doc);
        self.history.interrupt();
        self.caret_moved();
        cx.notify();
    }

    /// The selection as markdown — what a copy puts on the clipboard, and what
    /// a paste elsewhere reads back. Inside one fence, the code as it stands.
    fn selected_source(&self) -> Option<String> {
        if self.selection.is_collapsed() {
            return None;
        }
        if self.in_fence() {
            let (start, end) = self.selection.clamp(&self.doc).ordered();
            let code = self.doc.blocks[start.block].text_at(Part::Code)?;
            return Some(code.text[start.offset..end.offset].to_string());
        }
        Some({
            let mut slice = self.doc.slice(self.selection);
            slice.normalize_with(&self.marks);
            markdown::serialize_with(&slice, &self.marks)
        })
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(source) = self.selected_source() {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(source));
        }
    }

    fn cut(&mut self, _: &Cut, _: &mut Window, cx: &mut Context<Self>) {
        let Some(source) = self.selected_source() else {
            return;
        };
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(source));
        self.edit(EditKind::Structure, cx, |this| {
            let splice = this.doc.replace(this.selection, Text::default());
            this.selection = Selection::at(splice.caret.clamp(&this.doc));
            vec![Delta::Spliced(splice)]
        });
    }

    /// Markdown in, at the caret. A lone paragraph goes in as inline text with
    /// its marks; anything else arrives as blocks.
    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = cx.read_from_clipboard() else {
            return;
        };
        // Source mode included: the whole document is one fence there.
        if self.in_fence() {
            if let Some(text) = item.text() {
                self.paste_literal(&text, cx);
            }
            return;
        }
        // A picture before its text, because a clipboard carrying both is
        // carrying a name for the picture — which is not the picture. A
        // screenshot has a file name beside its bytes, and a file copied in a
        // file manager has its path beside the path itself.
        for entry in item.entries() {
            let placed = match entry {
                gpui::ClipboardEntry::Image(image) => self.paste_image(image, cx),
                gpui::ClipboardEntry::ExternalPaths(paths) => self.paste_paths(paths, cx),
                gpui::ClipboardEntry::String(_) => false,
            };
            if placed {
                return;
            }
        }
        let Some(source) = item.text() else {
            return;
        };
        let url = source.trim();
        if markdown::is_url(url) {
            return self.paste_url(url.to_string(), cx);
        }
        self.edit(EditKind::Structure, cx, |this| {
            let removed = this.selection;
            let before = this.doc.blocks.len();
            let head = this
                .doc
                .splice(removed, markdown::parse_with(&source, &this.marks));
            this.selection = Selection::at(head.clamp(&this.doc));
            vec![Delta::Spliced(Splice {
                removed,
                caret: head,
                blocks: this.doc.blocks.len() as isize - before as isize,
            })]
        });
    }

    /// Whether the selection starts and ends in one fence's code.
    fn in_fence(&self) -> bool {
        let (start, end) = self.selection.ordered();
        start.part == Part::Code && end.part == Part::Code && start.block == end.block
    }

    /// Put `text` in place of the selection as it stands, caret after it.
    fn paste_literal(&mut self, text: &str, cx: &mut Context<Self>) {
        self.edit(EditKind::Structure, cx, |this| {
            let splice = this.doc.replace(this.selection, Text::plain(text));
            this.selection = Selection::at(splice.caret.clamp(&this.doc));
            vec![Delta::Spliced(splice)]
        });
    }

    /// A URL is never spliced in as a block. It links whatever is selected, or
    /// lands as a link where the caret is — and only when the block it landed
    /// in held nothing else does it also offer to become a card, which is the
    /// one place a card would not eat a sentence.
    fn paste_url(&mut self, url: String, cx: &mut Context<Self>) {
        if self.in_fence() {
            return self.paste_literal(&url, cx);
        }
        // The one paste people expect to *not* overwrite what they chose.
        if !self.selection.is_collapsed() {
            return self.toggle_mark(Mark::Link(url), cx);
        }
        // A card needs a block with nothing else in it; a chip needs a body or
        // a cell to sit in. A fence holds its URL literally and offers neither.
        let at = self.cursor();
        let alone = at.part == Part::Body && self.caret_text().is_some_and(Text::is_empty);
        self.edit(EditKind::Structure, cx, |this| {
            let splice = this.doc.replace(this.selection, Text::link(&url));
            this.selection = Selection::at(splice.caret.clamp(&this.doc));
            vec![Delta::Spliced(splice)]
        });
        // A fence holds its URL literally and a caption cannot spell a mark, so
        // neither has a richer form to offer.
        if self.chrome.paste && !matches!(at.part, Part::Code | Part::Caption) {
            self.pasted = Some(link::Paste::open(at, url, alone));
            cx.notify();
        }
    }

    /// Answer the paste menu: leave the link, or turn its block into a card or
    /// the picture it points at.
    pub(super) fn confirm_paste(&mut self, choice: Choice, cx: &mut Context<Self>) {
        let Some(pasted) = self.pasted.take() else {
            return;
        };
        let ix = pasted.at.block;
        let card = |url, form| BlockKind::Bookmark { url, form };
        match choice {
            Choice::Dismiss => cx.notify(),
            // A chip with a line to itself is a block, which is what gives it
            // room for a favicon; inside a sentence it is a mark over the text
            // that is already there, and only the spelling changes.
            Choice::Chip if pasted.alone => self.turn_into(ix, card(pasted.url, Form::Chip), cx),
            Choice::Chip => self.edit(EditKind::Structure, cx, |this| {
                let end = Cursor {
                    offset: pasted.at.offset + pasted.url.len(),
                    ..pasted.at
                };
                let text = Text {
                    text: pasted.url.clone(),
                    marks: vec![markdown::MarkSpan {
                        range: 0..pasted.url.len(),
                        mark: Mark::Mention {
                            url: pasted.url,
                            form: markdown::Form::Chip,
                        },
                    }],
                };
                let splice = this.doc.replace(Selection::new(pasted.at, end), text);
                this.selection = Selection::at(splice.caret.clamp(&this.doc));
                vec![Delta::Spliced(splice)]
            }),
            Choice::Bookmark => self.turn_into(ix, card(pasted.url, Form::Auto), cx),
            Choice::Embed => self.turn_into(ix, card(pasted.url, Form::Embed), cx),
            Choice::Image => self.turn_into(
                ix,
                BlockKind::Image {
                    url: pasted.url,
                    alt: Text::default(),
                    width: None,
                },
                cx,
            ),
        }
    }

    /// Give a block over to the link it holds — a card, or the picture it
    /// points at.
    ///
    /// One step, not two: turning the block and giving the caret somewhere to
    /// go are one gesture, and undo has to agree.
    fn turn_into(&mut self, ix: usize, kind: BlockKind, cx: &mut Context<Self>) {
        self.edit(EditKind::Structure, cx, |this| {
            // The URL is the block's whole text and the new block shows it
            // already, so [`Doc::set_kind`] is given nothing to carry across —
            // otherwise a picture prints its own URL as its caption.
            let held = this.doc.blocks[ix]
                .text_at(Part::Body)
                .map_or(0, |text| text.text.len());
            this.doc.edit_at(Cursor::new(ix, Part::Body, 0), |text| {
                *text = Text::default()
            });
            this.doc.set_kind(ix, kind);
            // A caret goes where the block admits one, and otherwise carries on
            // in the block after it — a fresh one when it ends the document.
            let part = this.doc.blocks[ix].parts().first().copied();
            let at = match part {
                Some(part) => Cursor::new(ix, part, 0),
                None => {
                    if this.doc.blocks.len() <= ix + 1 {
                        this.doc
                            .blocks
                            .push(markdown::Block::new(BlockKind::Paragraph(Text::default())));
                    }
                    Cursor::new(ix + 1, Part::Body, 0)
                }
            };
            this.selection = Selection::at(at.clamp(&this.doc));
            vec![Delta::Spliced(Splice {
                removed: Selection::new(
                    Cursor::new(ix, Part::Body, 0),
                    Cursor::new(ix, Part::Body, held),
                ),
                caret: Cursor::new(ix, Part::Body, 0),
                blocks: 0,
            })]
        });
    }

    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(step) = self
            .history
            .undo(self.mode, &self.doc, self.selection, &self.anchors)
        {
            self.restore(step, cx);
        }
    }

    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(step) = self
            .history
            .redo(self.mode, &self.doc, self.selection, &self.anchors)
        {
            self.restore(step, cx);
        }
    }

    /// Put a whole moment back — document, caret and anchors together.
    ///
    /// The anchors come from the snapshot rather than from mapping, because a
    /// step back is not an edit: there is no delta between here and a document
    /// two hundred keystrokes ago.
    fn restore(&mut self, step: crate::history::Step, cx: &mut Context<Self>) {
        self.doc = step.doc;
        self.selection = step.selection.clamp(&self.doc);
        self.caret_moved();
        self.anchors = step.anchors;
        if step.mode != self.mode {
            self.mode = step.mode;
            self.dismiss_menus();
            cx.emit(EditorEvent::ModeChanged(step.mode));
        }
        cx.emit(EditorEvent::Changed);
        cx.notify();
    }

    /// Move the caret's block, children and all, and follow it.
    ///
    /// Public because a gutter handle and a menu row reach the same operation
    /// as the key does — one vocabulary, not three paths into [`Doc`].
    pub fn move_block(&mut self, ix: usize, delta: isize, cx: &mut Context<Self>) {
        if !self.blocks() {
            return;
        }
        self.edit(EditKind::Structure, cx, |this| {
            let caret = this.cursor();
            let at = this.doc.subtree(ix);
            let Some(to) = this.doc.move_block(ix, delta) else {
                return vec![];
            };
            // The caret rides along, keeping its depth within the subtree
            // that moved and its offset within its own text.
            let block = to + caret.block.saturating_sub(ix);
            this.selection = Selection::at(Cursor { block, ..caret }.clamp(&this.doc));
            vec![Delta::Moved { at, to: Some(to) }]
        });
    }

    pub fn duplicate_block(&mut self, ix: usize, cx: &mut Context<Self>) {
        if !self.blocks() {
            return;
        }
        self.edit(EditKind::Structure, cx, |this| {
            let span = this.doc.subtree(ix);
            let Some(copy) = this.doc.duplicate(ix) else {
                return vec![];
            };
            this.selection = Selection::at(Cursor::new(copy, Part::Body, 0).clamp(&this.doc));
            vec![Delta::Opened {
                at: copy,
                count: span.len(),
            }]
        });
    }

    pub fn remove_block(&mut self, ix: usize, cx: &mut Context<Self>) {
        if !self.blocks() {
            return;
        }
        self.edit(EditKind::Structure, cx, |this| {
            let at = this.doc.subtree(ix);
            this.doc.remove_block(ix);
            this.selection =
                Selection::at(Cursor::new(ix.saturating_sub(1), Part::Body, 0).clamp(&this.doc));
            vec![Delta::Moved { at, to: None }]
        });
    }

    /// Tag a fenced block with the language it holds, or `None` for plain.
    pub fn set_language(&mut self, ix: usize, language: Option<String>, cx: &mut Context<Self>) {
        if !self.blocks() {
            return;
        }
        self.edit(EditKind::Structure, cx, |this| {
            this.doc.set_language(ix, language);
            vec![]
        });
    }

    /// Turn the caret's block into `kind` — what the slash menu and the block
    /// menu both do.
    pub fn set_block(&mut self, ix: usize, kind: BlockKind, cx: &mut Context<Self>) {
        if !self.blocks() {
            return;
        }
        self.edit(EditKind::Structure, cx, |this| {
            this.doc.set_kind(ix, kind);
            this.selection = this.selection.clamp(&this.doc);
            vec![]
        });
    }

    /// Check or uncheck the task block at `ix`. Does nothing to a block that
    /// is not one.
    ///
    /// An edit like any other: one undo step, and the caret stays where it
    /// was, so toggling a box across the document does not move whatever is
    /// being typed.
    pub fn toggle_task(&mut self, ix: usize, cx: &mut Context<Self>) {
        if !self.blocks() {
            return;
        }
        let checked = match self.doc.blocks.get(ix).map(|block| &block.kind) {
            Some(BlockKind::Task { checked, .. }) => !checked,
            _ => return,
        };
        self.edit(EditKind::Structure, cx, |this| {
            if let Some(BlockKind::Task { checked: at, .. }) =
                this.doc.blocks.get_mut(ix).map(|block| &mut block.kind)
            {
                *at = checked;
            }
            vec![]
        });
        // Every other edit is made at the caret and owes it a reveal. This one
        // is made where a press landed, and the caret can be pages away: the
        // reveal `edit` asked for would scroll the box being checked off the
        // screen.
        self.reveal = false;
    }

    /// The task block whose checkbox `at` landed in, in window coordinates.
    ///
    /// The box that painted rather than the marker column: a press in the
    /// gutter beside it is a press on the row, and still places a caret.
    fn checkbox_at(&self, at: gpui::Point<gpui::Pixels>) -> Option<usize> {
        let ix = self.layouts.block_at(at)?;
        self.layouts
            .checkbox_bounds(ix)
            .is_some_and(|bounds| bounds.contains(&at))
            .then_some(ix)
    }

    /// The paragraph a document ending in a fence, a table, a rule or an image
    /// has no other way to grow: a fence swallows Enter, a cell and a caption
    /// have nowhere to put one, and a rule holds no caret at all. `false` when
    /// the last block ends in a body, which can carry on by itself.
    fn append_tail(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(last) = self.doc.blocks.len().checked_sub(1) else {
            return false;
        };
        if !self.blocks() {
            return false;
        }
        if self.doc.blocks[last].parts().last() == Some(&Part::Body) {
            return false;
        }
        self.edit(EditKind::Structure, cx, |this| {
            this.doc
                .blocks
                .push(markdown::Block::new(BlockKind::Paragraph(Text::default())));
            let ix = this.doc.blocks.len() - 1;
            this.selection = Selection::at(Cursor::new(ix, Part::Body, 0).clamp(&this.doc));
            vec![]
        });
        true
    }

    /// Press at `position` as if on the document: menus close, the editor
    /// takes focus, and the caret goes to the nearest place a caret can be —
    /// below the last block, above the first, or beside a line.
    ///
    /// For a host whose own frame around the editor should behave as the page. A
    /// press the editor's box already took is marked with
    /// [`Window::prevent_default`], and this ignores one so marked.
    pub fn press(
        &mut self,
        position: gpui::Point<gpui::Pixels>,
        click_count: usize,
        modifiers: gpui::Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if window.default_prevented() {
            return;
        }
        window.prevent_default();
        self.pressed(position, click_count, modifiers, window, cx);
    }

    /// Whether a press is being dragged: a selection, a lifted block or an
    /// image resize.
    fn in_drag(&self) -> bool {
        self.dragging || self.lifted.is_some() || self.resizing.is_some()
    }

    /// Follow a dragged pointer, wherever in the window it is.
    fn drag_to(&mut self, position: gpui::Point<gpui::Pixels>, cx: &mut Context<Self>) {
        // A lifted block follows the pointer.
        if let Some((from, _)) = self.lifted {
            if let Some(to) = self.layouts.block_at(position) {
                self.lifted = Some((from, to));
                cx.notify();
            }
            return;
        }
        // An image being resized follows the pointer the same way — the
        // document holds nothing until the handle is released.
        if let Some((ix, _)) = self.resizing {
            if let Some(width) = self.dragged_width(ix, position.x) {
                self.resizing = Some((ix, Some(width)));
                cx.notify();
            }
            return;
        }
        if self.dragging
            && let Some(hit) = self.layouts.hit(position)
        {
            self.selection = self.selection.extend_to(hit).clamp(&self.doc);
            cx.notify();
        }
    }

    fn pressed(
        &mut self,
        position: gpui::Point<gpui::Pixels>,
        click_count: usize,
        modifiers: gpui::Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        ui::popover::close_popup(self, cx, |this| &mut this.block_menu);
        ui::popover::close_popup(self, cx, |this| &mut this.language_menu);
        self.pasted = None;
        self.focus_handle.clone().focus(window, cx);
        // Ahead of the hit test, and returning without one: the
        // box is a control, and a caret dropped into the row
        // behind it would move the caret on every check.
        if let Some(ix) = self.checkbox_at(position) {
            self.toggle_task(ix, cx);
            return;
        }
        if self.tail_click(position, cx) {
            return;
        }
        let Some(hit) = self.layouts.hit(position) else {
            return cx.notify();
        };
        self.selection = match click_count {
            // Shift extends from wherever the anchor already is,
            // which is what makes click-then-shift-click a range.
            _ if modifiers.shift => self.selection.extend_to(hit),
            1 => Selection::at(hit),
            2 => Selection::new(hit.word_left(&self.doc), hit.word_right(&self.doc)),
            _ => Selection::new(hit.home(), hit.end(&self.doc)),
        }
        .clamp(&self.doc);
        self.dragging = click_count == 1 && !modifiers.shift;
        self.history.interrupt();
        self.caret_moved();
        // Only the editor sees the press, so only the editor can
        // say which anchor it landed on.
        if let Some(id) = self.anchor_at(position) {
            cx.emit(EditorEvent::AnchorActivated(id));
        }
        cx.notify();
    }

    /// A click past the end of the document. Without this the document has no
    /// end: the click snaps back into the block above it, and what gets typed
    /// lands inside the code the reader was trying to escape.
    fn tail_click(&mut self, at: gpui::Point<gpui::Pixels>, cx: &mut Context<Self>) -> bool {
        let Some(last) = self.doc.blocks.len().checked_sub(1) else {
            return false;
        };
        let Some(bounds) = self.layouts.block_bounds(last) else {
            return false;
        };
        at.y > bounds.origin.y + bounds.size.height && self.append_tail(cx)
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

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let layout = Layout::of(cx);
        let focused = self.focus_handle.is_focused(window);
        // The only place the blink starts: `caret_moved` drops the task, so the
        // next render brings it back in phase, lit beat first.
        if focused && ui::input::caret_blink(cx) {
            if self.blink.is_none() {
                self.start_blink(cx);
            }
        } else {
            self.blink = None;
            self.caret_on = true;
        }
        let selection = focused.then_some(self.selection);
        // gpui ends an outside file drag — left the window or released
        // elsewhere — without a drop here, so the indicator goes with it.
        if !cx.has_active_drag() {
            self.dropping = None;
        }

        // Typed text and IME reach an entity only through an input handler
        // registered during *paint*, against the bounds it should be anchored
        // to. There is no custom element here to do that from, so a zero-cost
        // canvas over the document supplies the paint phase. Without this the
        // key bindings still fire and nothing types.
        let handle = self.focus_handle.clone();
        let entity = cx.entity();
        let in_drag = self.in_drag();
        let input = canvas(
            |_, _, _| (),
            move |bounds, _, window, cx| {
                // `on_mouse_move` hears the pointer only over this box, and a
                // drag goes on past it. Registering it is paint's alone.
                if in_drag {
                    let entity = entity.clone();
                    window.on_mouse_event(move |event: &gpui::MouseMoveEvent, phase, _, cx| {
                        if phase == gpui::DispatchPhase::Bubble && event.dragging() {
                            entity.update(cx, |this, cx| this.drag_to(event.position, cx));
                        }
                    });
                }
                // The gutter handle is placed from positions recorded in window
                // coordinates, so the box they have to be measured against is
                // taken here — the one place that knows it.
                entity.update(cx, |this, _| {
                    this.origin = bounds.origin;
                    this.width = bounds.size.width;
                });
                window.handle_input(
                    &handle,
                    ElementInputHandler::new(bounds, entity.clone()),
                    cx,
                );
            },
        )
        .absolute()
        .size_full();

        // A tab stop, so the editor is reachable the same way every other
        // control in the library is.
        let handle = self.focus_handle.clone().tab_stop(true);

        div()
            // Stateful only so the pointer leaving can be heard: `on_hover` is
            // what tells the gutter handle to stop pointing at a block the
            // pointer left behind.
            .id("bezel-editor")
            // The mark is what keeps `tab`: without it traversal answers the
            // key first and the caret never sees it.
            .key_context(key_context())
            .track_focus(&handle)
            // Tracking focus does not take it. Without this, clicking into the
            // document blurs the editor instead of putting a caret in it, and
            // the caret vanishes on the first click.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    // The handle's own listener runs first and claims the
                    // press; without the flag this would close the menu it
                    // just opened. `ui::popover::Popup` solves it the same way.
                    if std::mem::take(&mut this.press_claimed) {
                        this.focus_handle.clone().focus(window, cx);
                    } else {
                        this.pressed(
                            event.position,
                            event.click_count,
                            event.modifiers,
                            window,
                            cx,
                        );
                    }
                    // What `press` reads to skip a press this box already
                    // took. It also stops gpui's own focus transfer, which runs
                    // after this listener, so both arms focus by hand.
                    window.prevent_default();
                }),
            )
            // The drag has to be tracked from the container rather than from a
            // payload: a text selection has nothing to carry, and gpui's drag
            // payload is for things being dropped somewhere.
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if !*hovered && this.hovered.take().is_some() {
                    cx.notify();
                }
            }))
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                // Ahead of every drag branch below, because the pointer's shape
                // is about where it *is* rather than about what it is doing.
                let over_text = this.layouts.over_text(event.position);
                if over_text != this.over_text {
                    this.over_text = over_text;
                    cx.notify();
                }
                // A drag in flight is followed by the window-wide listener
                // `render` registers; otherwise the pointer only decides which
                // block wears the handle.
                if this.in_drag() {
                    return;
                }
                let hovered = this.layouts.block_at(event.position);
                if hovered != this.hovered {
                    this.hovered = hovered;
                    cx.notify();
                }
            }))
            // Both, because a release can land anywhere on screen and only the
            // first fires over the editor. A resize left running would leave
            // its stand-in picture painted over the document for good.
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _: &gpui::MouseUpEvent, window, cx| {
                    this.dragging = false;
                    this.drop_resize(window, cx);
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseUpEvent, window, cx| {
                    this.dragging = false;
                    if this.drop_resize(window, cx) {
                        return;
                    }
                    let Some((from, to)) = this.lifted.take() else {
                        return;
                    };
                    if from == to {
                        // A press that never moved is a click, and a click on
                        // the handle is what opens the menu — unless that same
                        // press is what dismissed it, which the note taken on
                        // the way down is the only way to tell.
                        if !this.block_menu.take_press_was_open() {
                            this.block_menu.open((from, event.position));
                        }
                        return cx.notify();
                    }
                    this.edit(EditKind::Structure, cx, |this| {
                        // `move_block` steps one sibling at a time, so a drop
                        // several blocks away is that many steps. Bounded by
                        // the block count, which no drag can exceed.
                        let delta = if to > from { 1 } else { -1 };
                        let mut at = from;
                        // Each step is its own move, so each is its own delta —
                        // folding them into one would have to compose the
                        // hops, and they are already in order.
                        let mut deltas = Vec::new();
                        for _ in 0..this.doc.blocks.len() {
                            let span = this.doc.subtree(at);
                            let Some(next) = this.doc.move_block(at, delta) else {
                                break;
                            };
                            deltas.push(Delta::Moved {
                                at: span,
                                to: Some(next),
                            });
                            at = next;
                            if (delta > 0 && at >= to) || (delta < 0 && at <= to) {
                                break;
                            }
                        }
                        this.selection =
                            Selection::at(Cursor::new(at, Part::Body, 0).clamp(&this.doc));
                        deltas
                    });
                }),
            )
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(
                cx.listener(|this, _: &KillLine, _, cx| this.delete_to(true, Cursor::end, cx)),
            )
            .on_action(cx.listener(|this, _: &DeleteWordLeft, _, cx| {
                this.delete_to(false, Cursor::word_left, cx)
            }))
            .on_action(cx.listener(|this, _: &DeleteWordRight, _, cx| {
                this.delete_to(true, Cursor::word_right, cx)
            }))
            .on_action(cx.listener(|this, _: &DeleteToHome, _, cx| {
                this.delete_to(false, |at, _| at.home(), cx)
            }))
            .on_action(cx.listener(Self::split_block))
            .on_action(cx.listener(Self::soft_break))
            .on_action(cx.listener(Self::insert_paragraph))
            .on_action(cx.listener(Self::indent))
            .on_action(cx.listener(Self::increase_text_size))
            .on_action(cx.listener(Self::decrease_text_size))
            .on_action(cx.listener(Self::reset_text_size))
            .on_action(cx.listener(Self::outdent))
            .on_action(cx.listener(Self::dismiss))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::confirm_url))
            .on_action(cx.listener(Self::cancel_url))
            // A file crossing the document lights the same indicator a lifted
            // block does, so a drop from outside lands where it looks like it
            // will.
            .on_drag_move(cx.listener(
                |this, event: &gpui::DragMoveEvent<gpui::ExternalPaths>, _, cx| {
                    let at = event.event.position;
                    let over = event
                        .bounds
                        .contains(&at)
                        .then(|| this.layouts.block_at(at))
                        .flatten();
                    if over != this.dropping {
                        this.dropping = over;
                        cx.notify();
                    }
                },
            ))
            .on_drop(
                cx.listener(|this, paths: &gpui::ExternalPaths, window, cx| {
                    this.focus_handle.clone().focus(window, cx);
                    this.drop_paths(paths, cx);
                }),
            )
            .on_action(cx.listener(|this, _: &ToggleBold, _, cx| this.toggle_mark(Mark::Bold, cx)))
            .on_action(
                cx.listener(|this, _: &ToggleItalic, _, cx| this.toggle_mark(Mark::Italic, cx)),
            )
            .on_action(
                cx.listener(|this, _: &ToggleStrike, _, cx| this.toggle_mark(Mark::Strike, cx)),
            )
            .on_action(cx.listener(|this, _: &ToggleCode, _, cx| this.toggle_mark(Mark::Code, cx)))
            .on_action(cx.listener(|this, _: &ToggleHighlight, _, cx| {
                if this.marks.delimiter(HIGHLIGHT_MARK).is_some() {
                    this.toggle_mark(Mark::Custom(HIGHLIGHT_MARK.into()), cx);
                }
            }))
            .on_action(cx.listener(|this, _: &MoveBlockUp, _, cx| {
                this.move_block(this.cursor().block, -1, cx)
            }))
            .on_action(cx.listener(|this, _: &MoveBlockDown, _, cx| {
                this.move_block(this.cursor().block, 1, cx)
            }))
            .on_action(cx.listener(|this, _: &DuplicateBlock, _, cx| {
                this.duplicate_block(this.cursor().block, cx)
            }))
            .on_action(cx.listener(|this, _: &RemoveBlock, _, cx| {
                this.remove_block(this.cursor().block, cx)
            }))
            // Motion is one method with a `Cursor` function and an "extend"
            // flag, so a shift variant cannot drift from the key it shadows.
            .on_action(cx.listener(|this, _: &Left, _, cx| this.moved(false, Cursor::left, cx)))
            .on_action(cx.listener(|this, _: &Right, _, cx| this.moved(false, Cursor::right, cx)))
            .on_action(cx.listener(|this, _: &Up, _, cx| this.vertical(false, false, cx)))
            .on_action(cx.listener(|this, _: &Down, _, cx| this.vertical(true, false, cx)))
            .on_action(cx.listener(|this, _: &Home, _, cx| this.moved(false, line_home, cx)))
            .on_action(cx.listener(|this, _: &End, _, cx| this.moved(false, line_end, cx)))
            .on_action(cx.listener(|this, _: &DocumentStart, _, cx| {
                this.moved(false, |_, doc| Selection::all(doc).anchor, cx)
            }))
            .on_action(cx.listener(|this, _: &DocumentEnd, _, cx| {
                this.moved(false, |_, doc| Selection::all(doc).head, cx)
            }))
            .on_action(
                cx.listener(|this, _: &WordLeft, _, cx| this.moved(false, Cursor::word_left, cx)),
            )
            .on_action(
                cx.listener(|this, _: &WordRight, _, cx| this.moved(false, Cursor::word_right, cx)),
            )
            .on_action(
                cx.listener(|this, _: &SelectLeft, _, cx| this.moved(true, Cursor::left, cx)),
            )
            .on_action(
                cx.listener(|this, _: &SelectRight, _, cx| this.moved(true, Cursor::right, cx)),
            )
            .on_action(cx.listener(|this, _: &SelectUp, _, cx| this.vertical(false, true, cx)))
            .on_action(cx.listener(|this, _: &SelectDown, _, cx| this.vertical(true, true, cx)))
            .on_action(cx.listener(|this, _: &SelectHome, _, cx| this.moved(true, line_home, cx)))
            .on_action(cx.listener(|this, _: &SelectEnd, _, cx| this.moved(true, line_end, cx)))
            .on_action(cx.listener(|this, _: &SelectDocumentStart, _, cx| {
                this.moved(true, |_, doc| Selection::all(doc).anchor, cx)
            }))
            .on_action(cx.listener(|this, _: &SelectDocumentEnd, _, cx| {
                this.moved(true, |_, doc| Selection::all(doc).head, cx)
            }))
            .on_action(cx.listener(|this, _: &SelectWordLeft, _, cx| {
                this.moved(true, Cursor::word_left, cx)
            }))
            .on_action(cx.listener(|this, _: &SelectWordRight, _, cx| {
                this.moved(true, Cursor::word_right, cx)
            }))
            .w_full()
            // Text under the pointer, so the pointer says so — and only there,
            // or while a drag is still sweeping one out. The editor's box
            // reaches over its gutter, the margin beside a short line, a rule,
            // an image and a card, none of which a caret can be put into.
            // Where it has nothing to say it stays quiet rather than
            // overriding the page with an arrow of its own.
            .when(self.over_text || self.dragging, |el| {
                el.cursor(CursorStyle::IBeam)
            })
            // No focus ring. A ring says *widget*, and a document is not one —
            // the caret already paints only while focused, so a box around the
            // whole page is a second, louder signal for the same fact.
            .relative()
            .child(input)
            // The document is inset by the gutter so the handle has somewhere
            // to sit *inside* the editor. Outside it the handle is clipped by
            // any scrolling ancestor, and a pointer over it never reaches
            // `on_mouse_move`, which fires only while this element is the one
            // under the pointer.
            .child(
                div()
                    .w_full()
                    .pl(gpui::px(layout.text_inset))
                    .child(match self.mode {
                        // The source is one text, so it paints as one text —
                        // the same caret, the same clicks, no block chrome to
                        // suppress a piece at a time.
                        Mode::Source => markdown::render_source(
                            self.source_text(),
                            markdown::Editing {
                                selection,
                                caret_on: self.caret_on,
                                layouts: Some(&self.layouts),
                                typography: Some(markdown::Typography::of(cx).scaled(
                                    text_size::resolve(self.text_size, cx)
                                        / theme::base_text_size(),
                                )),
                                ..Default::default()
                            },
                            cx,
                        ),
                        Mode::Blocks => markdown::render_with(
                            &self.doc,
                            markdown::Editing {
                                selection,
                                caret_on: self.caret_on,
                                layouts: Some(&self.layouts),
                                annotations: &self.annotations(),
                                placeholder: focused.then(|| PLACEHOLDER.into()),
                                // A caret goes into the caption here, so it is always
                                // painted — an editor that could hide it would be
                                // hiding a place you can already be typing.
                                caption: markdown::Caption::Shown,
                                // The size is absolute, so the factor the ladder
                                // is already scaled by comes back out of it —
                                // otherwise the app's size and this one multiply.
                                typography: Some(markdown::Typography::of(cx).scaled(
                                    text_size::resolve(self.text_size, cx)
                                        / theme::base_text_size(),
                                )),
                                // The editor's own press hit-tests
                                // `checkbox_bounds`, which is what keeps a
                                // toggle in the undo history.
                                toggle: Some(markdown::Toggle::HitTested),
                                base: self.base.as_deref(),
                                ..Default::default()
                            },
                            window,
                            cx,
                        ),
                    }),
            )
            // Last, so the layouts it reads are this frame's rather than the
            // one before — children paint in order.
            .child(
                canvas(|_, _, _| (), {
                    let entity = cx.entity();
                    move |_, _, window, cx| {
                        entity.update(cx, |this, cx| {
                            this.reveal_caret(cx);
                            this.settle_handle(window, cx);
                        });
                    }
                })
                .absolute()
                .size(gpui::px(0.0)),
            )
            .children(self.slash_menu(&theme, cx))
            .children(self.paste_menu(&theme, cx))
            .children(self.url_prompt(&theme, cx))
            .children(self.image_target(cx))
            .children(self.resize_preview())
            .children(self.handle(focused, &theme, cx))
            .children(self.resize_handle(&theme, cx))
            .children(self.drop_indicator(&theme))
            .children(self.language_chip(&theme, cx))
            .children(self.block_menu(&theme, cx))
            .children(self.language_menu(&theme, cx))
    }
}
