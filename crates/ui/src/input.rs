//! [`TextField`] — a single-line text field with IME, selection and clipboard.
//!
//! Unlike the rest of this crate, a text field cannot be a plain function
//! returning a `Div`: editing needs state (content, selection, IME marked
//! range), a focus handle, and gpui's [`EntityInputHandler`]. So it is an
//! entity the caller holds — SwiftUI's `TextField` bound to `@State`, not a
//! stateless view.
//!
//! Ported from gpui's `examples/input.rs` (Apache-2.0), restyled onto
//! [`Theme`] tokens, with the key bindings **scoped to the field's key
//! context** rather than installed globally: a component library must not make
//! `cmd-a` mean "select all text" for the whole application.
//!
//! ```ignore
//! ui::input::init(cx);                      // once, at startup
//! let field = cx.new(|cx| TextField::new(cx).with_placeholder("Search…"));
//! // …then render it: .child(field.clone())
//! ```

use std::{borrow::Cow, ops::Range, time::Duration};

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, DispatchPhase, ElementId,
    ElementInputHandler, Entity, EntityInputHandler, EventEmitter, FocusHandle, Focusable, Global,
    GlobalElementId, KeyBinding, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, PaintQuad, Pixels, Point, SharedString, Style, Task, TextRun, UTF16Selection,
    UnderlineStyle, Window, WrappedLine, actions, div, fill, prelude::*, px, relative,
};
use unicode_segmentation::UnicodeSegmentation as _;

use theme::{HighlightKind, Metrics, SyntaxPalette, TextStyle, Theme};

mod edit;
mod element;
mod ime;
mod text;

pub use element::*;
pub use text::*;

actions!(
    bezel_text_field,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        SelectHome,
        SelectEnd,
        WordLeft,
        WordRight,
        SelectWordLeft,
        SelectWordRight,
        DeleteWordLeft,
        DeleteWordRight,
        DeleteToLineStart,
        DeleteToLineEnd,
        ShowCharacterPalette,
        Paste,
        Cut,
        Copy,
        Up,
        Down,
        SelectUp,
        SelectDown,
        InsertNewline,
        Undo,
        Redo,
    ]
);

/// How many undo steps a field keeps by default.
///
/// Steps, not keystrokes: a run of typing coalesces into one, so this is deeper
/// than it looks. A text field is not a document — nobody walks a search box
/// back through a long history — and the ceiling is what stops a long-lived
/// field accumulating snapshots forever. Override per field with
/// [`TextField::with_undo_limit`].
pub const DEFAULT_UNDO_LIMIT: usize = 10;

/// What a field reports, one per user action.
///
/// [`gpui::Context::observe`] fires on every `notify`, which includes repaints
/// nothing asked for — the caret's own blink among them. A picker refiltering
/// on those throws its cursor back to the first row twice a second. Subscribe
/// to this instead, and take only the half you need: a picker filters on
/// [`Self::Changed`], a composer reading the word behind the caret wants both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldEvent {
    /// The text is different.
    Changed,
    /// The text is the same and the caret is somewhere else.
    Moved,
}

/// Half the caret's blink period — the 500ms on, 500ms off macOS itself uses.
const BLINK: Duration = Duration::from_millis(500);

/// Whether carets blink. Absent means they do — a global nobody installed is
/// the default, not the opposite of it.
struct CaretBlink(bool);

impl Global for CaretBlink {}

/// Whether a caret blinks or is held solid. Read where a blink would start:
/// [`TextField`] here, and the editor's own caret.
pub fn caret_blink(cx: &App) -> bool {
    cx.try_global::<CaretBlink>().is_none_or(|blink| blink.0)
}

/// A caret held solid is still a caret — turning the blink off stops the task
/// and leaves the caret lit, never caught on the half of the beat that hides it.
pub fn set_caret_blink(blink: bool, cx: &mut App) {
    cx.set_global(CaretBlink(blink));
    cx.refresh_windows();
}

/// Width of the caret. Named because horizontal scrolling has to keep the caret
/// itself on screen, not merely the character before it.
const CARET_WIDTH: Pixels = px(2.);

/// The key context the field claims; bindings from [`init`] are scoped to it.
pub const KEY_CONTEXT: &str = "TextField";

/// Claimed *in addition* to [`KEY_CONTEXT`] by a multi-line field.
///
/// Vertical motion and `enter` hang off this rather than off every field,
/// because a single-line field is routinely nested inside something that has
/// already claimed those keys: [`crate::palette`] and [`crate::combobox`] both
/// bind `up`, `down`, `ctrl-n`, `ctrl-p` and `enter` to drive their lists, and
/// their query field sits *deeper* in the focus path — so binding those on
/// every `TextField` would win the dispatch and break list navigation in both.
pub const MULTILINE_KEY_CONTEXT: &str = "TextArea";

/// Install the bindings — [`bindings`], bound. Call once at startup.
pub fn init(cx: &mut App) {
    cx.bind_keys(bindings());
}

/// The field's keymap, as data, so an app can have it without having to
/// take it — see [`crate::keys`] for layering over it or taking a chord
/// away.
///
/// Every binding is scoped to [`KEY_CONTEXT`], so they are inert outside a
/// focused field and an app is free to bind the same chords elsewhere.
pub fn bindings() -> Vec<KeyBinding> {
    let mut bindings = Vec::new();
    let ctx = Some(KEY_CONTEXT);
    bindings.extend([
        // Character movement and editing, everywhere.
        KeyBinding::new("backspace", Backspace, ctx),
        KeyBinding::new("delete", Delete, ctx),
        KeyBinding::new("left", Left, ctx),
        KeyBinding::new("right", Right, ctx),
        KeyBinding::new("shift-left", SelectLeft, ctx),
        KeyBinding::new("shift-right", SelectRight, ctx),
        KeyBinding::new("home", Home, ctx),
        KeyBinding::new("end", End, ctx),
        KeyBinding::new("shift-home", SelectHome, ctx),
        KeyBinding::new("shift-end", SelectEnd, ctx),
    ]);

    // Multi-line only — see [`MULTILINE_KEY_CONTEXT`] for why these cannot be
    // bound on every field.
    let area = Some(MULTILINE_KEY_CONTEXT);
    bindings.extend([
        KeyBinding::new("enter", InsertNewline, area),
        KeyBinding::new("up", Up, area),
        KeyBinding::new("down", Down, area),
        KeyBinding::new("shift-up", SelectUp, area),
        KeyBinding::new("shift-down", SelectDown, area),
    ]);

    #[cfg(target_os = "macos")]
    bindings.extend([
        KeyBinding::new("cmd-a", SelectAll, ctx),
        KeyBinding::new("cmd-c", Copy, ctx),
        KeyBinding::new("cmd-x", Cut, ctx),
        KeyBinding::new("cmd-v", Paste, ctx),
        KeyBinding::new("cmd-z", Undo, ctx),
        KeyBinding::new("cmd-shift-z", Redo, ctx),
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, ctx),
        // cmd = line, option = word: the macOS convention.
        KeyBinding::new("cmd-left", Home, ctx),
        KeyBinding::new("cmd-right", End, ctx),
        KeyBinding::new("cmd-shift-left", SelectHome, ctx),
        KeyBinding::new("cmd-shift-right", SelectEnd, ctx),
        KeyBinding::new("alt-left", WordLeft, ctx),
        KeyBinding::new("alt-right", WordRight, ctx),
        KeyBinding::new("alt-shift-left", SelectWordLeft, ctx),
        KeyBinding::new("alt-shift-right", SelectWordRight, ctx),
        KeyBinding::new("cmd-backspace", DeleteToLineStart, ctx),
        KeyBinding::new("alt-backspace", DeleteWordLeft, ctx),
        KeyBinding::new("alt-delete", DeleteWordRight, ctx),
        // The emacs bindings macOS honours in every native text field.
        KeyBinding::new("ctrl-a", Home, ctx),
        KeyBinding::new("ctrl-e", End, ctx),
        KeyBinding::new("ctrl-b", Left, ctx),
        KeyBinding::new("ctrl-f", Right, ctx),
        KeyBinding::new("ctrl-h", Backspace, ctx),
        KeyBinding::new("ctrl-d", Delete, ctx),
        KeyBinding::new("ctrl-k", DeleteToLineEnd, ctx),
    ]);

    // `C-n`/`C-p` are emacs' vertical motion and macOS `NSTextView` natives
    // both — the two tests a chord has to pass to earn a binding here.
    #[cfg(target_os = "macos")]
    bindings.extend([
        KeyBinding::new("ctrl-n", Down, area),
        KeyBinding::new("ctrl-p", Up, area),
    ]);

    #[cfg(not(target_os = "macos"))]
    bindings.extend([
        KeyBinding::new("ctrl-a", SelectAll, ctx),
        KeyBinding::new("ctrl-c", Copy, ctx),
        KeyBinding::new("ctrl-x", Cut, ctx),
        KeyBinding::new("ctrl-v", Paste, ctx),
        // ctrl = word on Windows/Linux, where there is no line modifier.
        KeyBinding::new("ctrl-left", WordLeft, ctx),
        KeyBinding::new("ctrl-right", WordRight, ctx),
        KeyBinding::new("ctrl-shift-left", SelectWordLeft, ctx),
        KeyBinding::new("ctrl-shift-right", SelectWordRight, ctx),
        KeyBinding::new("ctrl-backspace", DeleteWordLeft, ctx),
        KeyBinding::new("ctrl-delete", DeleteWordRight, ctx),
        KeyBinding::new("ctrl-z", Undo, ctx),
        KeyBinding::new("ctrl-shift-z", Redo, ctx),
        KeyBinding::new("ctrl-y", Redo, ctx),
    ]);

    bindings
}

/// What case a field holds its text in.
///
/// Applied to every edit rather than to the string on its way out, so what is
/// painted, what `content()` returns and what a caller stores are one string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Case {
    /// Whatever was typed.
    #[default]
    Mixed,
    /// Upper, for a field naming something a heading is drawn from.
    Upper,
}

impl Case {
    /// `text` in this case. Borrowed where the case leaves it alone, which
    /// is the default and every keystroke through it.
    fn apply(self, text: &str) -> Cow<'_, str> {
        match self {
            Self::Mixed => Cow::Borrowed(text),
            Self::Upper => Cow::Owned(text.to_uppercase()),
        }
    }
}

/// What shape the field takes.
///
/// Editing is identical across all three — every action works on the content
/// and a byte range, and none of them cares where the lines break. What this
/// decides is the box: how tall it is, whether text wraps, and with that what
/// `enter` and a pasted newline are allowed to mean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Shape {
    /// One line, no wrapping. `enter` does not insert; a pasted newline becomes
    /// a space rather than silently truncating what was pasted.
    #[default]
    Line,
    /// Exactly `rows` lines tall, wrapping, scrolling past that.
    Rows(usize),
    /// Wraps and grows with the content between `min` and `max` rows, then
    /// scrolls — the composer shape.
    Grow { min: usize, max: usize },
}

impl Shape {
    /// Whether newlines are content. The single branch every editing policy
    /// hangs off, so it is asked once rather than matched in each caller.
    fn is_multiline(self) -> bool {
        !matches!(self, Self::Line)
    }
}

/// A point the field can be returned to.
///
/// A whole snapshot rather than a diff: a field holds a sentence, not a file,
/// and `SharedString` clones are a refcount bump. A rope and a transaction log
/// is what an editor needs and would be the wrong machinery here.
#[derive(Clone)]
struct Snapshot {
    content: SharedString,
    selection: Range<usize>,
    reversed: bool,
}

/// Which way an edit went, so a run of the same kind can coalesce into one
/// undo step instead of giving the text back a character at a time.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EditKind {
    Insert,
    Delete,
}

/// A text field. [`Shape`] decides whether it is one line or many; everything
/// else about it is the same either way.
pub struct TextField {
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    shape: Shape,
    /// The case every edit is put through — see [`Case`].
    case: Case,
    selected_range: Range<usize>,
    selection_reversed: bool,
    /// The IME composition range (underlined while composing).
    marked_range: Option<Range<usize>>,
    /// One entry per hard newline; each wraps into rows of its own. Empty until
    /// the first paint.
    last_layout: Vec<WrappedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
    /// The column vertical motion is trying to keep, in pixels from the left of
    /// the row. Held across a run of up/down so that walking through a short
    /// line and out the other side returns to the column you started in, and
    /// dropped by anything horizontal — which is every other way the caret
    /// moves, so [`TextField::move_to`] and [`TextField::select_to`] clear it
    /// and the vertical handlers put it back.
    goal_x: Option<Pixels>,
    /// How far the text is scrolled inside the box. Clamped every frame,
    /// because the content it is measured against changes under it.
    ///
    /// Both axes, though only ever one at a time: a wrapped field's lines are
    /// shaped to the box width so they cannot overflow sideways, and a
    /// single-line field is exactly one row tall so it cannot overflow
    /// downwards. The clamp falls out of that and needs no test for shape.
    scroll: Point<Pixels>,
    history: crate::history::SnapshotHistory<Snapshot>,
    /// The kind of the last edit and the offset it left the caret at, which is
    /// what decides whether the next edit joins that group or starts a new one.
    /// Adjacency rather than a pause, so there is no timing threshold to invent.
    last_edit: Option<(EditKind, usize)>,
    /// A context this field claims *in addition* to [`KEY_CONTEXT`] and
    /// [`MULTILINE_KEY_CONTEXT`] — see [`Self::with_key_context`].
    key_context: Option<SharedString>,
    /// Whether the field paints a box around itself. Off for a field that
    /// stands in for text already in place — a rename in a list row, where the
    /// box would be a second frame inside the row's own.
    frame: bool,
    /// What the text is set in. Its leading is a multiple of the painted size,
    /// so the whole line box follows [`theme::set_base_text_size`].
    metrics: Metrics,
    /// Which half of the blink the caret is in. Flipped by [`Self::start_blink`].
    caret_on: bool,
    /// The blink, alive only while the field holds focus.
    blink: Option<Task<()>>,
    /// Set by anything that moves the caret, cleared once a frame has scrolled
    /// it back into view.
    ///
    /// Without the flag the wheel could never win: following the caret
    /// unconditionally would snap the view back to it on the very next frame,
    /// so scrolling away to read would be impossible.
    follow_caret: bool,
    /// Byte ranges to paint in a syntax colour, in document order — see
    /// [`Self::set_spans`]. Empty for every field that is prose.
    spans: Vec<(Range<usize>, HighlightKind)>,
    /// Byte ranges washed behind the text — see [`Self::set_matches`].
    matches: Vec<Range<usize>>,
}

impl EventEmitter<FieldEvent> for TextField {}

impl TextField {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            // A field is a tab stop from birth; a stateless control needs
            // `focus::focusable` because its handle lives in the caller.
            focus_handle: cx.focus_handle().tab_stop(true),
            frame: true,
            content: "".into(),
            placeholder: "".into(),
            shape: Shape::Line,
            case: Case::default(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: Vec::new(),
            last_bounds: None,
            is_selecting: false,
            goal_x: None,
            scroll: Point::default(),
            history: crate::history::SnapshotHistory::new(DEFAULT_UNDO_LIMIT),
            last_edit: None,
            key_context: None,
            metrics: TextStyle::Body.into(),
            caret_on: true,
            blink: None,
            follow_caret: false,
            spans: Vec::new(),
            matches: Vec::new(),
        }
    }

    /// How many undo steps to keep. App-wide configuration would be a gpui
    /// global alongside [`crate::input::init`], not a [`Theme`] field — the
    /// theme is rebuilt on every light/dark switch, which would quietly reset
    /// anything behavioural parked in it.
    pub fn with_undo_limit(mut self, limit: usize) -> Self {
        self.history.set_limit(limit);
        self
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Claim an extra key context on this field, so an app can bind a key
    /// *here* that it does not want bound in every other field.
    ///
    /// The composer case, and the reason this exists: `enter` sends a message
    /// and `shift-enter` breaks a line, while the notes field two panels over
    /// still takes `enter` as a newline. gpui resolves a keystroke to the
    /// binding whose context matches **deepest** in the focus path, and nothing
    /// is deeper than the focused field — so a container around it cannot win
    /// `enter`, however it is bound. Rebinding [`MULTILINE_KEY_CONTEXT`]
    /// globally would win, and would take the newline away from every other
    /// multi-line field in the app. A context of the field's own is the only
    /// thing that is both deep enough and narrow enough.
    ///
    /// ```ignore
    /// const COMPOSER: &str = "Composer";
    /// cx.bind_keys([
    ///     KeyBinding::new("enter", Send, Some(COMPOSER)),
    ///     KeyBinding::new("shift-enter", input::InsertNewline, Some(COMPOSER)),
    /// ]);
    /// let field = cx.new(|cx| {
    ///     TextField::new(cx)
    ///         .with_shape(Shape::Grow { min: 3, max: 12 })
    ///         .with_key_context(COMPOSER)
    /// });
    /// ```
    pub fn with_key_context(mut self, context: impl Into<SharedString>) -> Self {
        self.key_context = Some(context.into());
        self
    }

    /// Draw without the box, padding and focus ring. The caller owns the
    /// spacing then, and owns showing that the field is focused.
    pub fn with_frame(mut self, frame: bool) -> Self {
        self.frame = frame;
        self
    }

    /// Set the text in something other than body copy —
    /// `Typography::of(cx).h1` sets a field the way a document sets its own
    /// heading.
    pub fn with_metrics(mut self, metrics: Metrics) -> Self {
        self.metrics = metrics;
        self
    }

    /// Update typography without replacing the editing state.
    pub fn set_metrics(&mut self, metrics: Metrics, cx: &mut Context<Self>) {
        if self.metrics != metrics {
            self.metrics = metrics;
            cx.notify();
        }
    }

    pub fn with_shape(mut self, shape: Shape) -> Self {
        self.shape = shape;
        self
    }

    pub fn with_case(mut self, case: Case) -> Self {
        self.case = case;
        self
    }

    /// The case from here on. What the field already holds is left as it is:
    /// the text came from somewhere else, and a caller switching the case of a
    /// shared field would otherwise rewrite the name it is showing.
    pub fn set_case(&mut self, case: Case) {
        self.case = case;
    }

    pub fn case(&self) -> Case {
        self.case
    }

    pub fn shape(&self) -> Shape {
        self.shape
    }

    pub fn content(&self) -> &SharedString {
        &self.content
    }

    /// The syntax spans this field is painting — see [`Self::set_spans`].
    pub fn spans(&self) -> &[(Range<usize>, HighlightKind)] {
        &self.spans
    }

    /// Replace the content, putting the cursor at the end.
    pub fn set_content(&mut self, content: impl Into<SharedString>, cx: &mut Context<Self>) {
        let normalized = normalize(&content.into(), self.shape);
        self.content = self.case.apply(&normalized).into_owned().into();
        // Colours describe text this field no longer holds.
        self.spans.clear();
        self.matches.clear();
        // A programmatic reset is not something the user did, so there is
        // nothing here for them to undo back past.
        self.history.clear();
        self.last_edit = None;
        let end = self.content.len();
        self.selected_range = end..end;
        self.marked_range = None;
        cx.emit(FieldEvent::Changed);
        cx.notify();
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.set_content("", cx);
    }

    /// Paint these byte ranges in syntax colours, in document order. Whatever
    /// they leave uncovered stays the field's own text colour, so a language
    /// nothing can colour is simply no spans at all.
    ///
    /// This field holds text, not a document: it does not parse, and it does
    /// not keep the spans in step with edits. The caller recomputes them —
    /// [`FieldEvent::Changed`] is the signal — and until it does, the frames in
    /// between paint the ranges it last handed over. Stale ones are dropped
    /// rather than shifted, so the worst a late recolour looks like is a word
    /// in the wrong colour for a frame.
    ///
    /// [`set_content`](Self::set_content) clears them: replacing the text
    /// replaces what the colours were about.
    pub fn set_spans(&mut self, spans: Vec<(Range<usize>, HighlightKind)>, cx: &mut Context<Self>) {
        self.spans = spans;
        cx.notify();
    }

    /// Wash these byte ranges behind the text, as search matches. Not kept in
    /// step with edits, and cleared by [`Self::set_content`], the same as
    /// [`Self::set_spans`]. A range past the end or off a char boundary is
    /// not painted.
    pub fn set_matches(&mut self, matches: Vec<Range<usize>>, cx: &mut Context<Self>) {
        self.matches = matches;
        cx.notify();
    }

    /// Select `range` and scroll it into view, with the caret at its end.
    /// Clamped to the content and to char boundaries.
    pub fn select(&mut self, range: Range<usize>, cx: &mut Context<Self>) {
        let end = floor_boundary(&self.content, range.end);
        let start = floor_boundary(&self.content, range.start.min(end));
        self.selected_range = start..end;
        self.selection_reversed = false;
        self.marked_range = None;
        self.goal_x = None;
        self.caret_moved();
        cx.notify();
    }

    /// [`Self::with_placeholder`] after construction, for a hint that follows
    /// something else — the field naming whichever agent, file or channel is
    /// selected rather than being rebuilt each time one is.
    pub fn set_placeholder(
        &mut self,
        placeholder: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.placeholder = placeholder.into();
        cx.notify();
    }

    /// The caret moved: bring it back into view, and drop the blink so the next
    /// render starts a fresh one. Without the reset the caret would blink
    /// through your own typing, which reads as a dropped keystroke.
    fn caret_moved(&mut self) {
        self.follow_caret = true;
        self.blink = None;
    }

    /// Blink the caret for as long as the field holds focus.
    fn start_blink(&mut self, cx: &mut Context<Self>) {
        self.caret_on = true;
        self.blink = Some(cx.spawn(async move |field, cx| {
            loop {
                cx.background_executor().timer(BLINK).await;
                let flipped = field.update(cx, |field, cx| {
                    field.caret_on = !field.caret_on;
                    cx.notify();
                });
                if flipped.is_err() {
                    break;
                }
            }
        }));
    }

    /// Where the caret is, as a byte offset into [`Self::content`].
    ///
    /// What a caller needs to read the text *behind* the caret: the `#` an
    /// autocomplete triggers on, the word a lookup would act on. With a
    /// selection this is the moving end, which is where typing would land.
    pub fn cursor(&self) -> usize {
        self.cursor_offset()
    }

    /// Where a byte offset sits on screen — the bounds of the row it is on, in
    /// window coordinates.
    ///
    /// The anchor for anything that hangs off a position *in the text* rather
    /// than off the field: a mention picker under the `#` that opened it, in a
    /// box that is also growing a row at a time. Pass it to
    /// [`crate::popover::menu_at`], which takes a window point.
    ///
    /// `None` until the field has painted once — this is measured off the
    /// shaped layout, and there is none before then.
    pub fn offset_bounds(&self, offset: usize) -> Option<Bounds<Pixels>> {
        self.row_bounds(self.text_origin()?, offset..offset, self.line_height())
    }

    /// The rectangle `range` spans, starting from the row it opens on, relative
    /// to `origin`.
    ///
    /// The IME's candidate panel and [`Self::offset_bounds`] are the same
    /// question asked by two callers, so they ask it here — computed apart they
    /// would answer differently the first time one of them forgot the scroll.
    fn row_bounds(
        &self,
        origin: Point<Pixels>,
        range: Range<usize>,
        line_height: Pixels,
    ) -> Option<Bounds<Pixels>> {
        let start = position_for_offset(&self.last_layout, range.start, line_height)?;
        let end = position_for_offset(&self.last_layout, range.end, line_height)?;
        Some(Bounds::from_corners(
            origin + start,
            origin + gpui::point(end.x, end.y + line_height),
        ))
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.goal_x = None;
        self.caret_moved();
        cx.emit(FieldEvent::Moved);
        cx.notify()
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    /// The row height every mapping between a screen point and a byte offset
    /// walks by: this field's own, which is what its shaped lines were laid
    /// out at — see [`TextField::render`], which sets it on the box.
    ///
    /// Never `window.line_height()`. That answers for whatever text style is
    /// current, and outside this field's own paint there is none of it on the
    /// stack — a click handler is told the window's default instead. Walk the
    /// rows at that stride and the caret lands a line or two above the one
    /// under the pointer, further out the lower you click, until the last
    /// lines of a full box cannot be reached at all.
    fn line_height(&self) -> Pixels {
        px(self.metrics.line_height())
    }

    /// Where the shaped text starts on screen: the box, moved up by the scroll.
    /// Every mapping between a screen point and a byte offset goes through it.
    fn text_origin(&self) -> Option<Point<Pixels>> {
        Some(self.last_bounds?.origin - self.scroll)
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>, line_height: Pixels) -> usize {
        if self.content.is_empty() || self.last_layout.is_empty() {
            return 0;
        }
        let Some(origin) = self.text_origin() else {
            return 0;
        };
        offset_for_position(&self.last_layout, position - origin, line_height)
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.goal_x = None;
        self.caret_moved();
        if self.selection_reversed {
            self.selected_range.start = offset
        } else {
            self.selected_range.end = offset
        };
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.emit(FieldEvent::Moved);
        cx.notify()
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        offset_to_utf16(&self.content, offset)
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        range_to_utf16(&self.content, range.clone())
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        range_from_utf16(&self.content, range_utf16.clone())
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        previous_boundary(&self.content, offset)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        next_boundary(&self.content, offset)
    }
}

impl Focusable for TextField {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
