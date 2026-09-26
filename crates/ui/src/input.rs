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

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx)
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx)
        }
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx)
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(line_start(&self.content, self.cursor_offset()), cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(line_end(&self.content, self.cursor_offset()), cx);
    }

    fn select_home(&mut self, _: &SelectHome, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(line_start(&self.content, self.cursor_offset()), cx);
    }

    fn select_end(&mut self, _: &SelectEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(line_end(&self.content, self.cursor_offset()), cx);
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(-1, false, cx);
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(1, false, cx);
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(-1, true, cx);
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(1, true, cx);
    }

    /// Move the caret `rows` rows, keeping the goal column.
    ///
    /// Rows are *visual*, so this walks soft wraps one at a time rather than
    /// jumping a whole paragraph — the opposite call from `ctrl-a`/`ctrl-e`,
    /// and the right one: down should land where it looks like it will.
    ///
    /// Geometry rather than arithmetic on line numbers, so wrapped rows and hard
    /// newlines are the same case and neither needs counting.
    fn vertical(&mut self, rows: i32, extend: bool, cx: &mut Context<Self>) {
        if self.last_layout.is_empty() {
            return;
        }
        let line_height = self.line_height();
        let Some(at) = position_for_offset(&self.last_layout, self.cursor_offset(), line_height)
        else {
            return;
        };
        let goal = self.goal_x.unwrap_or(at.x);
        let target = at.y + line_height * rows as f32;
        // Off the top is the start of the text and off the bottom is its end —
        // what every native field does with up/down on the first/last row.
        let offset = if target < px(0.) {
            0
        } else {
            offset_for_position(&self.last_layout, gpui::point(goal, target), line_height)
        };

        if extend {
            self.select_to(offset, cx);
        } else {
            self.move_to(offset, cx);
        }
        // Both of the above clear the goal; this is the one motion that keeps it.
        self.goal_x = Some(goal);
    }

    /// `enter`. Guarded as well as bound to [`MULTILINE_KEY_CONTEXT`], because
    /// an action can also be dispatched directly.
    fn insert_newline(&mut self, _: &InsertNewline, window: &mut Window, cx: &mut Context<Self>) {
        if self.shape.is_multiline() {
            self.replace_text_in_range(None, "\n", window, cx);
        }
    }

    fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(
            previous_word_boundary(&self.content, self.cursor_offset()),
            cx,
        );
    }

    fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(next_word_boundary(&self.content, self.cursor_offset()), cx);
    }

    fn select_word_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(
            previous_word_boundary(&self.content, self.cursor_offset()),
            cx,
        );
    }

    fn select_word_right(&mut self, _: &SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(next_word_boundary(&self.content, self.cursor_offset()), cx);
    }

    /// Every delete-by-unit action is "extend the selection over the unit, then
    /// replace it" — so a non-empty selection always wins, matching how every
    /// native field behaves.
    fn delete_word_left(
        &mut self,
        _: &DeleteWordLeft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_range.is_empty() {
            self.select_to(
                previous_word_boundary(&self.content, self.cursor_offset()),
                cx,
            );
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete_word_right(
        &mut self,
        _: &DeleteWordRight,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_range.is_empty() {
            self.select_to(next_word_boundary(&self.content, self.cursor_offset()), cx);
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete_to_line_start(
        &mut self,
        _: &DeleteToLineStart,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_range.is_empty() {
            self.select_to(line_start(&self.content, self.cursor_offset()), cx);
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete_to_line_end(
        &mut self,
        _: &DeleteToLineEnd,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_range.is_empty() {
            self.select_to(line_end(&self.content, self.cursor_offset()), cx);
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let prev = self.previous_boundary(self.cursor_offset());
            if self.cursor_offset() == prev {
                return;
            }
            self.select_to(prev, cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let next = self.next_boundary(self.cursor_offset());
            if self.cursor_offset() == next {
                return;
            }
            self.select_to(next, cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_selecting = true;
        let offset = self.index_for_mouse_position(event.position, self.line_height());
        if event.modifiers.shift {
            self.select_to(offset, cx);
        } else {
            self.move_to(offset, cx)
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    /// Scrolling is the one thing that moves the view without moving the caret,
    /// so it deliberately does not set `follow_caret` — the next frame clamps
    /// this, and the caret is left wherever it was.
    fn on_scroll_wheel(
        &mut self,
        event: &gpui::ScrollWheelEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let delta = event.delta.pixel_delta(self.line_height());
        self.scroll.x = (self.scroll.x - delta.x).max(px(0.));
        self.scroll.y = (self.scroll.y - delta.y).max(px(0.));
        cx.notify();
    }

    /// Carry the run to wherever the pointer went. Driven by the window rather
    /// than by the box — see [`TextFieldElement::paint`] — so `line_height` is
    /// passed in: the field's own is only current while the field is painting.
    fn drag_to(&mut self, position: Point<Pixels>, line_height: Pixels, cx: &mut Context<Self>) {
        if !self.is_selecting {
            return;
        }
        let offset = self.index_for_mouse_position(position, line_height);
        // A pointer crossing a character is the event worth having; the twenty
        // samples it takes to cross one are not.
        if offset != self.cursor_offset() {
            self.select_to(offset, cx);
        }
    }

    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &normalize(&text, self.shape), window, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx)
        }
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            content: self.content.clone(),
            selection: self.selected_range.clone(),
            reversed: self.selection_reversed,
        }
    }

    fn restore(&mut self, point: Snapshot, cx: &mut Context<Self>) {
        self.content = point.content;
        self.selected_range = point.selection;
        self.selection_reversed = point.reversed;
        self.marked_range = None;
        // The next edit must not join whatever group was open before.
        self.last_edit = None;
        self.caret_moved();
        cx.emit(FieldEvent::Changed);
        cx.notify();
    }

    /// Record the state before an edit, unless that edit continues the group the
    /// last one opened. Contiguity is the whole rule: the same kind of edit,
    /// starting where the caret was left. Type a run and it is one step; move
    /// the caret, or switch from typing to deleting, and the next one starts a
    /// group of its own.
    fn push_undo(&mut self, kind: EditKind, at: usize) {
        let before = (!joins_group(self.last_edit, kind, at)).then(|| self.snapshot());
        self.history.record(before);
    }

    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        let current = self.snapshot();
        if let Some(point) = self.history.undo(|| current) {
            self.restore(point, cx);
        }
    }

    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        let current = self.snapshot();
        if let Some(point) = self.history.redo(|| current) {
            self.restore(point, cx);
        }
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

// ---------------------------------------------------------------------------
// Pure offset math — the part with the sharp edges, kept free of gpui so it
// can be unit-tested.
// ---------------------------------------------------------------------------

/// UTF-16 offset (what the platform IME speaks) → byte offset.
pub fn offset_from_utf16(text: &str, offset: usize) -> usize {
    let mut utf8_offset = 0;
    let mut utf16_count = 0;
    for ch in text.chars() {
        if utf16_count >= offset {
            break;
        }
        utf16_count += ch.len_utf16();
        utf8_offset += ch.len_utf8();
    }
    utf8_offset
}

/// Byte offset → UTF-16 offset.
pub fn offset_to_utf16(text: &str, offset: usize) -> usize {
    let mut utf16_offset = 0;
    let mut utf8_count = 0;
    for ch in text.chars() {
        if utf8_count >= offset {
            break;
        }
        utf8_count += ch.len_utf8();
        utf16_offset += ch.len_utf16();
    }
    utf16_offset
}

/// Platform range → byte range, clamped to character boundaries in `text`.
pub fn range_from_utf16(text: &str, range: Range<usize>) -> Range<usize> {
    offset_from_utf16(text, range.start)..offset_from_utf16(text, range.end)
}

/// Byte range → platform range.
pub fn range_to_utf16(text: &str, range: Range<usize>) -> Range<usize> {
    offset_to_utf16(text, range.start)..offset_to_utf16(text, range.end)
}

/// An IME selection is relative to the replacement text, not the document.
pub fn composition_selection(
    text: &str,
    start: usize,
    selection: Option<Range<usize>>,
) -> Range<usize> {
    let range = selection
        .map(|range| range_from_utf16(text, range))
        .unwrap_or(text.len()..text.len());
    start + range.start..start + range.end
}

/// Previous *grapheme* boundary, so arrow keys and backspace step over a flag
/// emoji or a combining mark as one unit instead of splitting it into pieces
/// that render as garbage.
pub fn previous_boundary(text: &str, offset: usize) -> usize {
    text.grapheme_indices(true)
        .rev()
        .find_map(|(idx, _)| (idx < offset).then_some(idx))
        .unwrap_or(0)
}

/// Next grapheme boundary; clamps to the end of the text.
pub fn next_boundary(text: &str, offset: usize) -> usize {
    text.grapheme_indices(true)
        .find_map(|(idx, _)| (idx > offset).then_some(idx))
        .unwrap_or(text.len())
}

/// Whether an edit continues the group the last one opened, rather than
/// starting an undo step of its own.
///
/// Two conditions, both structural: the same kind of edit, landing where the
/// last one left the caret. Deliberately not "within N milliseconds" — a time
/// threshold is a number nobody has measured, and adjacency is what actually
/// distinguishes a run of typing from a fresh thought somewhere else.
pub fn joins_group(last: Option<(EditKind, usize)>, kind: EditKind, at: usize) -> bool {
    last.is_some_and(|(last_kind, offset)| last_kind == kind && at == offset)
}

/// The line breaks a field of this shape is allowed to hold.
///
/// CRLF is folded to LF whatever the shape: `shape_text` splits on `\n` alone,
/// so a surviving `\r` shapes as a glyph and puts every offset after it out by
/// one. A single-line field then keeps the text but not the breaks — a pasted
/// newline becomes a space rather than silently truncating what was pasted.
///
/// The invariant this buys: a [`Shape::Line`] field's content never contains a
/// newline, so nothing downstream has to ask whether it might.
pub fn normalize(text: &str, shape: Shape) -> String {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    if shape.is_multiline() {
        text
    } else {
        text.replace('\n', " ")
    }
}

/// Start of the logical line holding `offset` — the byte after the previous
/// newline.
///
/// Logical, not visual: with soft wrapping these two readings diverge, and
/// `ctrl-a` here goes to the start of the whole paragraph rather than stopping
/// at the wrap. That is emacs' `C-a`, and a deliberate divergence from macOS
/// `NSTextView`, which stops at the visual row. On text with no newline — every
/// [`Shape::Line`] field — the two are identical.
pub fn line_start(text: &str, offset: usize) -> usize {
    text[..offset].rfind('\n').map_or(0, |at| at + 1)
}

/// End of the logical line holding `offset` — the byte before the next newline.
pub fn line_end(text: &str, offset: usize) -> usize {
    text[offset..]
        .find('\n')
        .map_or(text.len(), |at| offset + at)
}

/// A word-bound segment counts as a word if it has any alphanumeric content;
/// whitespace and punctuation runs are the things word motion skips over.
fn is_word(segment: &str) -> bool {
    segment.chars().any(char::is_alphanumeric)
}

/// Start of the word at or before `offset` — option-left.
///
/// Word units are Unicode word bounds (UAX#29), not space-delimited runs. In
/// practice that means `foo.bar` and `foo_bar` are ONE word — a dot or
/// underscore between letters does not break — while `a-b`, `path/to/file` and
/// `foo, bar` do break. Good defaults for identifiers and paths, and verified
/// against the segmenter rather than assumed (see tests).
pub fn previous_word_boundary(text: &str, offset: usize) -> usize {
    text.split_word_bound_indices()
        .filter(|(start, _)| *start < offset)
        .rfind(|(_, segment)| is_word(segment))
        .map(|(start, _)| start)
        .unwrap_or(0)
}

/// End of the word at or after `offset` — option-right.
pub fn next_word_boundary(text: &str, offset: usize) -> usize {
    text.split_word_bound_indices()
        .filter(|(start, segment)| start + segment.len() > offset)
        .find(|(_, segment)| is_word(segment))
        .map(|(start, segment)| start + segment.len())
        .unwrap_or(text.len())
}

/// What gets painted, and whether it is the placeholder — which is the only
/// reason the colour differs.
fn display_text(field: &TextField) -> (SharedString, bool) {
    if field.content.is_empty() {
        (field.placeholder.clone(), true)
    } else {
        (field.content.clone(), false)
    }
}

/// One run per span, the text between them in the field's own colour.
///
/// The runs a field paints [`TextField::set_spans`] as, public for the same
/// reason [`next_boundary`] is: anything shaping the same text with the same
/// palette wants the same answer, and this is where it is decided.
///
/// The spans are whatever the caller last handed over, which may describe text
/// this frame no longer holds — see [`TextField::set_spans`]. A range that runs
/// past the end, overlaps the one before it, or cuts a character in half is
/// dropped rather than shifting the runs after it: shaping needs the lengths to
/// add up to the text, and a colour that is wrong for one frame is cheaper than
/// a panic.
///
/// `base.len` is ignored: every run is measured off `text`.
pub fn coloured(
    text: &str,
    spans: &[(Range<usize>, HighlightKind)],
    base: &TextRun,
    palette: &SyntaxPalette,
) -> Vec<TextRun> {
    if spans.is_empty() {
        return vec![TextRun {
            len: text.len(),
            ..base.clone()
        }];
    }
    let mut runs = Vec::with_capacity(spans.len() * 2 + 1);
    let mut at = 0;
    for (range, kind) in spans {
        if range.start < at
            || range.end <= range.start
            || range.end > text.len()
            || !text.is_char_boundary(range.start)
            || !text.is_char_boundary(range.end)
        {
            continue;
        }
        if range.start > at {
            runs.push(TextRun {
                len: range.start - at,
                ..base.clone()
            });
        }
        runs.push(TextRun {
            len: range.end - range.start,
            color: palette.color(*kind),
            ..base.clone()
        });
        at = range.end;
    }
    if at < text.len() {
        runs.push(TextRun {
            len: text.len() - at,
            ..base.clone()
        });
    }
    runs
}

/// The IME composition range underlined, so the user can see what is still
/// provisional. Each run is cut at the range's edges and the pieces inside it
/// take the line, in whatever colour they already had.
pub fn underlined(runs: Vec<TextRun>, marked: &Range<usize>) -> Vec<TextRun> {
    let mut out = Vec::with_capacity(runs.len() + 2);
    let mut at = 0;
    for run in runs {
        let end = at + run.len;
        for (start, stop, mark) in [
            (at, end.min(marked.start), false),
            (at.max(marked.start), end.min(marked.end), true),
            (at.max(marked.end), end, false),
        ] {
            if stop <= start {
                continue;
            }
            out.push(TextRun {
                len: stop - start,
                underline: mark.then(|| UnderlineStyle {
                    color: Some(run.color),
                    thickness: px(1.0),
                    wavy: false,
                }),
                ..run.clone()
            });
        }
        at = end;
    }
    out
}

// ---------------------------------------------------------------------------
// Line geometry. `shape_text` returns one `WrappedLine` per hard newline, each
// wrapping into rows of its own; gpui resolves positions *within* a line, so
// everything here is walking that list and nothing re-implements shaping.
// ---------------------------------------------------------------------------

/// Each shaped line with the byte offset it starts at. `shape_text` splits on
/// `\n` and drops the separator, so each line starts one byte past the last.
fn lines_from(lines: &[WrappedLine]) -> impl Iterator<Item = (usize, &WrappedLine)> {
    lines.iter().scan(0usize, |start, line| {
        let at = *start;
        *start = at + line.len() + 1;
        Some((at, line))
    })
}

/// Every visual row: the byte range it covers and its top edge, relative to the
/// text origin. A wrap boundary resolves to a byte index exactly the way
/// `WrappedLineLayout::position_for_index` does it internally — that mapping is
/// not exposed, and selection needs one quad per row.
fn rows(lines: &[WrappedLine], line_height: Pixels) -> Vec<(Range<usize>, Pixels)> {
    let mut out = Vec::new();
    let mut top = px(0.);
    for (start, line) in lines_from(lines) {
        let mut row_start = start;
        for boundary in line.wrap_boundaries() {
            let at = start + line.runs()[boundary.run_ix].glyphs[boundary.glyph_ix].index;
            out.push((row_start..at, top));
            row_start = at;
            top += line_height;
        }
        out.push((row_start..start + line.len(), top));
        top += line_height;
    }
    out
}

/// Byte offset → position relative to the text origin.
fn position_for_offset(
    lines: &[WrappedLine],
    offset: usize,
    line_height: Pixels,
) -> Option<Point<Pixels>> {
    let mut top = px(0.);
    for (start, line) in lines_from(lines) {
        if offset <= start + line.len() {
            let local = line.position_for_index(offset.saturating_sub(start), line_height)?;
            return Some(gpui::point(local.x, local.y + top));
        }
        top += line.size(line_height).height;
    }
    None
}

/// Position relative to the text origin → the closest byte offset.
fn offset_for_position(
    lines: &[WrappedLine],
    position: Point<Pixels>,
    line_height: Pixels,
) -> usize {
    let mut top = px(0.);
    let mut last = 0;
    for (start, line) in lines_from(lines) {
        let height = line.size(line_height).height;
        last = start + line.len();
        if position.y < top + height {
            let local = gpui::point(position.x, position.y - top);
            let (Ok(index) | Err(index)) = line.closest_index_for_position(local, line_height);
            return start + index;
        }
        top += height;
    }
    last
}

/// The char boundary at or before `offset`, clamped to the end of `text`.
fn floor_boundary(text: &str, offset: usize) -> usize {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

/// The selection as one rect per visual row, relative to the text origin.
///
/// A row's left edge is always x=0, so a continuation row is taken from there
/// rather than by looking the offset up — at a soft wrap the two rows share a
/// byte offset, and the lookup resolves it to the end of the earlier row.
fn selection_rows(
    lines: &[WrappedLine],
    range: &Range<usize>,
    line_height: Pixels,
) -> Vec<Bounds<Pixels>> {
    rows(lines, line_height)
        .into_iter()
        .filter(|(row, _)| range.start <= row.end && range.end >= row.start)
        .filter_map(|(row, top)| {
            let left = if range.start <= row.start {
                px(0.)
            } else {
                position_for_offset(lines, range.start, line_height)?.x
            };
            let right = position_for_offset(lines, range.end.min(row.end), line_height)?.x;
            (right > left).then(|| {
                Bounds::from_corners(
                    gpui::point(left, top),
                    gpui::point(right, top + line_height),
                )
            })
        })
        .collect()
}

impl EntityInputHandler for TextField {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        // Every edit lands here — typing, deleting, cut, paste, and the IME
        // *committing*. Not `replace_and_mark_text_in_range`, which is the
        // composing path: provisional text must not become undo steps, or every
        // keystroke of Chinese input would be one.
        let kind = if new_text.is_empty() {
            EditKind::Delete
        } else {
            EditKind::Insert
        };
        // A delete grows leftwards, so its group continues at the range's end;
        // an insert continues at its start.
        self.push_undo(
            kind,
            if new_text.is_empty() {
                range.end
            } else {
                range.start
            },
        );

        // Cased here rather than on the way out: the caret is placed from this
        // string's length, and a case that changes it (`ß` uppercases to two
        // bytes) would leave the caret off by the difference.
        let new_text = self.case.apply(new_text);
        self.content =
            (self.content[0..range.start].to_owned() + &new_text + &self.content[range.end..])
                .into();
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.marked_range.take();
        self.last_edit = Some((kind, self.selected_range.end));
        self.caret_moved();
        cx.emit(FieldEvent::Changed);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        // The composing text is cased too, so what an IME is showing is what
        // committing it will leave behind.
        let new_text = self.case.apply(new_text);
        self.content =
            (self.content[0..range.start].to_owned() + &new_text + &self.content[range.end..])
                .into();
        self.marked_range =
            (!new_text.is_empty()).then(|| range.start..range.start + new_text.len());
        self.selected_range =
            composition_selection(&new_text, range.start, new_selected_range_utf16);
        self.selection_reversed = false;

        self.caret_moved();
        cx.emit(FieldEvent::Changed);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.range_from_utf16(&range_utf16);
        // The IME panel anchors under the composing text, so this has to be the
        // row that text is on, not the whole field. `bounds` is what
        // `last_bounds` is set from, so this is the same origin
        // [`TextField::offset_bounds`] measures from.
        self.row_bounds(bounds.origin - self.scroll, range, self.line_height())
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        self.last_bounds?.localize(&point)?;
        let origin = self.text_origin()?;
        let offset = offset_for_position(&self.last_layout, point - origin, self.line_height());
        Some(self.offset_to_utf16(offset))
    }
}

impl Focusable for TextField {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TextField {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The only place the blink starts: `caret_moved` drops the task, so the
        // next render brings it back in phase, solid beat first.
        if self.focus_handle.is_focused(_window) && caret_blink(cx) {
            if self.blink.is_none() {
                self.start_blink(cx);
            }
        } else {
            self.blink = None;
            self.caret_on = true;
        }
        let theme = Theme::of(cx);
        let mut key_context = gpui::KeyContext::default();
        key_context.add(KEY_CONTEXT);
        if self.shape.is_multiline() {
            key_context.add(MULTILINE_KEY_CONTEXT);
        }
        if let Some(extra) = self.key_context.clone() {
            key_context.add(extra);
        }
        div()
            .key_context(key_context)
            .track_focus(&self.focus_handle(cx))
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::select_home))
            .on_action(cx.listener(Self::select_end))
            .on_action(cx.listener(Self::word_left))
            .on_action(cx.listener(Self::word_right))
            .on_action(cx.listener(Self::select_word_left))
            .on_action(cx.listener(Self::select_word_right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::insert_newline))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::delete_word_left))
            .on_action(cx.listener(Self::delete_word_right))
            .on_action(cx.listener(Self::delete_to_line_start))
            .on_action(cx.listener(Self::delete_to_line_end))
            .on_action(cx.listener(Self::show_character_palette))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_scroll_wheel(cx.listener(Self::on_scroll_wheel))
            .w_full()
            .when(self.frame, |field| {
                field
                    .px(px(10.0))
                    .py(px(7.0))
                    .rounded(px(Theme::button_radius()))
                    .bg(theme.input_bg)
                    .border_1()
                    .border_color(if self.focus_handle.is_focused(_window) {
                        theme.ring
                    } else {
                        theme.border
                    })
            })
            .text_size(px(self.metrics.size()))
            .font_weight(self.metrics.weight)
            .line_height(px(self.metrics.line_height()))
            .text_color(theme.text)
            .child(TextFieldElement { field: cx.entity() })
    }
}

/// Paints the shaped lines plus selection and caret. A custom element because
/// all three are geometry derived from the shaped text, which only exists after
/// layout.
struct TextFieldElement {
    field: Entity<TextField>,
}

struct FieldPrepaint {
    lines: Vec<WrappedLine>,
    /// Top-left of the text, which is the box moved up by the scroll offset.
    origin: Point<Pixels>,
    cursor: Option<PaintQuad>,
    /// One quad per visual row each match covers.
    matches: Vec<PaintQuad>,
    /// One quad per visual row the selection covers.
    selection: Vec<PaintQuad>,
}

impl IntoElement for TextFieldElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextFieldElement {
    type RequestLayoutState = ();
    type PrepaintState = FieldPrepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        let field = self.field.read(cx);
        // The field's own, here as everywhere: sizing the box off one height
        // and hit-testing it at another is how a click lands on the wrong row.
        let line_height = field.line_height();
        let shape = field.shape;

        let (min, max) = match shape {
            Shape::Line => {
                style.size.height = line_height.into();
                return (window.request_layout(style, [], cx), ());
            }
            Shape::Rows(rows) => {
                style.size.height = (line_height * rows.max(1) as f32).into();
                return (window.request_layout(style, [], cx), ());
            }
            // Growing needs the row count, and the row count needs shaping at
            // the width layout is still deciding — which is exactly what a
            // measured layout is for.
            Shape::Grow { min, max } => (min.max(1), max.max(min.max(1))),
        };

        let text = display_text(field).0;
        let id = window.request_measured_layout(style, move |known, available, window, _cx| {
            let text_style = window.text_style();
            let font_size = text_style.font_size.to_pixels(window.rem_size());
            // Prefer the width layout has already settled on. Taffy also probes
            // with min/max-content, where there is no width to wrap against —
            // and counting rows off unwrapped text under-reports them, which
            // would size the box for fewer lines than it goes on to paint.
            let wrap_width = known.width.or(match available.width {
                gpui::AvailableSpace::Definite(width) => Some(width),
                _ => None,
            });
            let run = TextRun {
                len: text.len(),
                font: text_style.font(),
                color: text_style.color,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let count = window
                .text_system()
                .shape_text(text.clone(), font_size, &[run], wrap_width, None)
                .map(|lines| {
                    lines
                        .iter()
                        .map(|line| line.wrap_boundaries().len() + 1)
                        .sum::<usize>()
                })
                .unwrap_or(1);
            gpui::size(
                wrap_width.unwrap_or(px(0.)),
                line_height * count.clamp(min, max) as f32,
            )
        });
        (id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> FieldPrepaint {
        let theme = Theme::of(cx).clone();
        let field = self.field.read(cx);
        let selected_range = field.selected_range.clone();
        let cursor = field.cursor_offset();
        let shape = field.shape;
        let marked_range = field.marked_range.clone();
        let matches: Vec<_> = field
            .matches
            .iter()
            .filter(|range| {
                range.start <= range.end
                    && field.content.is_char_boundary(range.start)
                    && field.content.is_char_boundary(range.end)
            })
            .cloned()
            .collect();
        let scrolled = field.scroll;
        let follow_caret = field.follow_caret;
        let style = window.text_style();

        let (text, is_placeholder) = display_text(field);
        let text_color = if is_placeholder {
            theme.text_faint
        } else {
            style.color
        };

        let run = TextRun {
            len: text.len(),
            font: style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        // Syntax first, then the IME composition range underlined over the top
        // of it: both cut the text into runs, and the underline has to land on
        // whatever colour is already there.
        let runs = if is_placeholder {
            vec![run]
        } else {
            coloured(&text, &field.spans, &run, &theme.syntax)
        };
        let runs = match marked_range.as_ref() {
            Some(marked) => underlined(runs, marked),
            None => runs,
        };

        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = field.line_height();
        // A single line never wraps: it scrolls sideways instead, so shaping it
        // against the field's width would fold it into rows nothing can reach.
        let wrap_width = shape.is_multiline().then_some(bounds.size.width);
        let lines = window
            .text_system()
            .shape_text(text, font_size, &runs, wrap_width, None)
            .map(|lines| lines.into_vec())
            .unwrap_or_default();

        // Clamp every frame, not just when scrolling: the content this is
        // measured against shrinks under it — delete the last line while parked
        // at the bottom and an unclamped offset leaves the box showing nothing.
        //
        // Only one axis is ever live. Wrapped lines are shaped to the box width,
        // so `max.x` is zero for a multi-line field; a single line is one row
        // tall, so `max.y` is zero for a single-line one. Neither needs asking
        // which shape it is.
        let content_height: Pixels = lines.iter().map(|l| l.size(line_height).height).sum();
        let content_width = lines.iter().map(|l| l.width()).fold(px(0.), Pixels::max);
        let max = gpui::point(
            (content_width - bounds.size.width).max(px(0.)),
            (content_height - bounds.size.height).max(px(0.)),
        );
        let mut scroll = gpui::point(
            scrolled.x.clamp(px(0.), max.x),
            scrolled.y.clamp(px(0.), max.y),
        );
        if follow_caret && let Some(at) = position_for_offset(&lines, cursor, line_height) {
            if at.y < scroll.y {
                scroll.y = at.y;
            } else if at.y + line_height > scroll.y + bounds.size.height {
                scroll.y = at.y + line_height - bounds.size.height;
            }
            // The caret is the thing being kept in view, so it is its own width
            // that has to clear the right edge — not the character before it.
            if at.x < scroll.x {
                scroll.x = at.x;
            } else if at.x + CARET_WIDTH > scroll.x + bounds.size.width {
                scroll.x = at.x + CARET_WIDTH - bounds.size.width;
            }
            scroll.x = scroll.x.clamp(px(0.), max.x);
            scroll.y = scroll.y.clamp(px(0.), max.y);
        }
        self.field.update(cx, |field, _| {
            field.scroll = scroll;
            field.follow_caret = false;
        });
        let origin = bounds.origin - scroll;

        let matches = if is_placeholder {
            Vec::new()
        } else {
            matches
                .iter()
                .flat_map(|range| selection_rows(&lines, range, line_height))
                .map(|rect| {
                    fill(
                        Bounds::new(origin + rect.origin, rect.size),
                        theme.warning.opacity(0.25),
                    )
                })
                .collect()
        };

        let (selection, cursor) = if selected_range.is_empty() {
            let at = position_for_offset(&lines, cursor, line_height).unwrap_or_default();
            (
                Vec::new(),
                Some(fill(
                    // The font's size rather than the line's: leading is not
                    // the caret's to fill.
                    Bounds::new(
                        origin + at + gpui::point(px(0.), (line_height - font_size) / 2.),
                        gpui::size(CARET_WIDTH, font_size),
                    ),
                    theme.caret,
                )),
            )
        } else {
            (
                selection_rows(&lines, &selected_range, line_height)
                    .into_iter()
                    .map(|rect| {
                        fill(
                            Bounds::new(origin + rect.origin, rect.size),
                            theme.selection,
                        )
                    })
                    .collect(),
                None,
            )
        };

        FieldPrepaint {
            lines,
            origin,
            cursor,
            matches,
            selection,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.field.read(cx).focus_handle.clone();
        let caret_on = self.field.read(cx).caret_on;
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.field.clone()),
            cx,
        );
        let line_height = self.field.read(cx).line_height();
        // A drag that leaves the box is still a drag. `on_mouse_move` on the
        // field's own div fires only while the box is the thing under the
        // pointer, so a run dragged past the edge froze at the last character
        // inside it — and the last line of a full box was unreachable, since
        // reaching it means passing the edge. This is the window's own move,
        // which arrives wherever the pointer went.
        //
        // Registered every frame because that is the contract: the listener is
        // cleared with the frame that installed it.
        let dragged = self.field.clone();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _window, cx| {
            // The button being down is what makes this a drag. `is_selecting`
            // is only cleared on the release, so a press whose release went
            // somewhere we never heard about would otherwise leave a plain
            // hover dragging the run around.
            if phase != DispatchPhase::Bubble || !event.dragging() {
                return;
            }
            dragged.update(cx, |field, cx| {
                field.drag_to(event.position, line_height, cx);
            });
        });
        let lines = std::mem::take(&mut prepaint.lines);
        let matches = std::mem::take(&mut prepaint.matches);
        let selection = std::mem::take(&mut prepaint.selection);
        let cursor = prepaint.cursor.take();
        let origin = prepaint.origin;

        // Scrolled text runs past the box in both directions, so everything the
        // field draws is masked to it — text, selection and caret alike.
        window.with_content_mask(Some(gpui::ContentMask::new(bounds)), |window| {
            for quad in matches.into_iter().chain(selection) {
                window.paint_quad(quad);
            }

            let mut top = origin;
            for line in &lines {
                line.paint(top, line_height, gpui::TextAlign::Left, None, window, cx)
                    .ok();
                top.y += line.size(line_height).height;
            }

            // The caret only exists while focused — an unfocused field showing
            // one reads as two cursors on screen.
            if focus_handle.is_focused(window)
                && caret_on
                && let Some(cursor) = cursor
            {
                window.paint_quad(cursor);
            }
        });

        self.field.update(cx, |field, _| {
            field.last_layout = lines;
            field.last_bounds = Some(bounds);
        });
    }
}
