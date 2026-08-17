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
//! bezel_ui::input::init(cx);                      // once, at startup
//! let field = cx.new(|cx| TextField::new(cx).with_placeholder("Search…"));
//! // …then render it: .child(field.clone())
//! ```

use std::ops::Range;

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, FocusHandle, Focusable, GlobalElementId, KeyBinding, LayoutId, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, SharedString, Style,
    TextRun, UTF16Selection, UnderlineStyle, Window, WrappedLine, actions, div, fill, prelude::*,
    px, relative,
};
use unicode_segmentation::UnicodeSegmentation as _;

use bezel_theme::Theme;

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

/// Install the default key bindings. Call once at startup.
///
/// Every binding is scoped to [`KEY_CONTEXT`], so they are inert outside a
/// focused field and an app is free to bind the same chords elsewhere.
///
/// **Optional.** This is a convenience, not a requirement: every action above
/// is a public type, so an app that wants its own keymap simply does not call
/// this and binds what it likes instead —
///
/// ```ignore
/// use bezel_ui::input::{self, Home, KEY_CONTEXT};
/// cx.bind_keys([KeyBinding::new("ctrl-a", Home, Some(KEY_CONTEXT))]);
/// ```
///
/// It is all-or-nothing, so taking the clipboard defaults while replacing the
/// motion ones means rebinding the lot. That is deliberate until something
/// needs finer grain.
pub fn init(cx: &mut App) {
    let ctx = Some(KEY_CONTEXT);
    cx.bind_keys([
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
    cx.bind_keys([
        KeyBinding::new("enter", InsertNewline, area),
        KeyBinding::new("up", Up, area),
        KeyBinding::new("down", Down, area),
        KeyBinding::new("shift-up", SelectUp, area),
        KeyBinding::new("shift-down", SelectDown, area),
    ]);

    #[cfg(target_os = "macos")]
    cx.bind_keys([
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
    cx.bind_keys([
        KeyBinding::new("ctrl-n", Down, area),
        KeyBinding::new("ctrl-p", Up, area),
    ]);

    #[cfg(not(target_os = "macos"))]
    cx.bind_keys([
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
    ]);
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
enum EditKind {
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
    /// Points to return to, oldest first. Bounded by `undo_limit`: the field
    /// outlives a lot of typing, and an unbounded history of a growing string
    /// is a slow leak nothing ever reclaims.
    undo: std::collections::VecDeque<Snapshot>,
    /// Undone points, newest last. Cleared by any fresh edit — the usual
    /// model, and the only one where redo cannot resurrect a branch the text
    /// has already diverged from.
    redo: Vec<Snapshot>,
    undo_limit: usize,
    /// The kind of the last edit and the offset it left the caret at, which is
    /// what decides whether the next edit joins that group or starts a new one.
    /// Adjacency rather than a pause, so there is no timing threshold to invent.
    last_edit: Option<(EditKind, usize)>,
    /// Set by anything that moves the caret, cleared once a frame has scrolled
    /// it back into view.
    ///
    /// Without the flag the wheel could never win: following the caret
    /// unconditionally would snap the view back to it on the very next frame,
    /// so scrolling away to read would be impossible.
    follow_caret: bool,
}

impl TextField {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            // A field is a tab stop from birth; a stateless control needs
            // `focus::focusable` because its handle lives in the caller.
            focus_handle: cx.focus_handle().tab_stop(true),
            content: "".into(),
            placeholder: "".into(),
            shape: Shape::Line,
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: Vec::new(),
            last_bounds: None,
            is_selecting: false,
            goal_x: None,
            scroll: Point::default(),
            undo: std::collections::VecDeque::new(),
            redo: Vec::new(),
            undo_limit: DEFAULT_UNDO_LIMIT,
            last_edit: None,
            follow_caret: false,
        }
    }

    /// How many undo steps to keep. App-wide configuration would be a gpui
    /// global alongside [`crate::input::init`], not a [`Theme`] field — the
    /// theme is rebuilt on every light/dark switch, which would quietly reset
    /// anything behavioural parked in it.
    pub fn with_undo_limit(mut self, limit: usize) -> Self {
        self.undo_limit = limit;
        self
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn with_shape(mut self, shape: Shape) -> Self {
        self.shape = shape;
        self
    }

    pub fn shape(&self) -> Shape {
        self.shape
    }

    pub fn content(&self) -> &SharedString {
        &self.content
    }

    /// Replace the content, putting the cursor at the end.
    pub fn set_content(&mut self, content: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.content = normalize(&content.into(), self.shape).into();
        // A programmatic reset is not something the user did, so there is
        // nothing here for them to undo back past.
        self.undo.clear();
        self.redo.clear();
        self.last_edit = None;
        let end = self.content.len();
        self.selected_range = end..end;
        self.marked_range = None;
        cx.notify();
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.set_content("", cx);
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

    fn up(&mut self, _: &Up, window: &mut Window, cx: &mut Context<Self>) {
        self.vertical(-1, false, window, cx);
    }

    fn down(&mut self, _: &Down, window: &mut Window, cx: &mut Context<Self>) {
        self.vertical(1, false, window, cx);
    }

    fn select_up(&mut self, _: &SelectUp, window: &mut Window, cx: &mut Context<Self>) {
        self.vertical(-1, true, window, cx);
    }

    fn select_down(&mut self, _: &SelectDown, window: &mut Window, cx: &mut Context<Self>) {
        self.vertical(1, true, window, cx);
    }

    /// Move the caret `rows` rows, keeping the goal column.
    ///
    /// Rows are *visual*, so this walks soft wraps one at a time rather than
    /// jumping a whole paragraph — the opposite call from `ctrl-a`/`ctrl-e`,
    /// and the right one: down should land where it looks like it will.
    ///
    /// Geometry rather than arithmetic on line numbers, so wrapped rows and hard
    /// newlines are the same case and neither needs counting.
    fn vertical(&mut self, rows: i32, extend: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.last_layout.is_empty() {
            return;
        }
        let line_height = window.line_height();
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_selecting = true;
        let offset = self.index_for_mouse_position(event.position, window.line_height());
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let delta = event.delta.pixel_delta(window.line_height());
        self.scroll.x = (self.scroll.x - delta.x).max(px(0.));
        self.scroll.y = (self.scroll.y - delta.y).max(px(0.));
        cx.notify();
    }

    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_selecting {
            let offset = self.index_for_mouse_position(event.position, window.line_height());
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
        self.follow_caret = true;
        cx.notify();
    }

    /// Record the state before an edit, unless that edit continues the group the
    /// last one opened. Contiguity is the whole rule: the same kind of edit,
    /// starting where the caret was left. Type a run and it is one step; move
    /// the caret, or switch from typing to deleting, and the next one starts a
    /// group of its own.
    fn push_undo(&mut self, kind: EditKind, at: usize) {
        if !joins_group(self.last_edit, kind, at) {
            self.undo.push_back(self.snapshot());
            while self.undo.len() > self.undo_limit {
                self.undo.pop_front();
            }
        }
        self.redo.clear();
    }

    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        let Some(point) = self.undo.pop_back() else {
            return;
        };
        self.redo.push(self.snapshot());
        self.restore(point, cx);
    }

    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        let Some(point) = self.redo.pop() else {
            return;
        };
        self.undo.push_back(self.snapshot());
        self.restore(point, cx);
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.goal_x = None;
        self.follow_caret = true;
        cx.notify()
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
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
        self.follow_caret = true;
        if self.selection_reversed {
            self.selected_range.start = offset
        } else {
            self.selected_range.end = offset
        };
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify()
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        offset_from_utf16(&self.content, offset)
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        offset_to_utf16(&self.content, offset)
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
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
fn offset_from_utf16(text: &str, offset: usize) -> usize {
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
fn offset_to_utf16(text: &str, offset: usize) -> usize {
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

/// Previous *grapheme* boundary, so arrow keys and backspace step over a flag
/// emoji or a combining mark as one unit instead of splitting it into pieces
/// that render as garbage.
fn previous_boundary(text: &str, offset: usize) -> usize {
    text.grapheme_indices(true)
        .rev()
        .find_map(|(idx, _)| (idx < offset).then_some(idx))
        .unwrap_or(0)
}

/// Next grapheme boundary; clamps to the end of the text.
fn next_boundary(text: &str, offset: usize) -> usize {
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
fn joins_group(last: Option<(EditKind, usize)>, kind: EditKind, at: usize) -> bool {
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
fn normalize(text: &str, shape: Shape) -> String {
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
fn line_start(text: &str, offset: usize) -> usize {
    text[..offset].rfind('\n').map_or(0, |at| at + 1)
}

/// End of the logical line holding `offset` — the byte before the next newline.
fn line_end(text: &str, offset: usize) -> usize {
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
fn previous_word_boundary(text: &str, offset: usize) -> usize {
    text.split_word_bound_indices()
        .filter(|(start, _)| *start < offset)
        .rfind(|(_, segment)| is_word(segment))
        .map(|(start, _)| start)
        .unwrap_or(0)
}

/// End of the word at or after `offset` — option-right.
fn next_word_boundary(text: &str, offset: usize) -> usize {
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
        // keystroke of Japanese input would be one.
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

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.marked_range.take();
        self.last_edit = Some((kind, self.selected_range.end));
        self.follow_caret = true;
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

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        self.marked_range =
            (!new_text.is_empty()).then(|| range.start..range.start + new_text.len());
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .map(|new_range| new_range.start + range.start..new_range.end + range.end)
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());

        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let line_height = window.line_height();
        let range = self.range_from_utf16(&range_utf16);
        // The IME panel anchors under the composing text, so this has to be the
        // row that text is on, not the whole field.
        let origin = bounds.origin - self.scroll;
        let start = position_for_offset(&self.last_layout, range.start, line_height)?;
        let end = position_for_offset(&self.last_layout, range.end, line_height)?;
        Some(Bounds::from_corners(
            origin + start,
            origin + gpui::point(end.x, end.y + line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        self.last_bounds?.localize(&point)?;
        let origin = self.text_origin()?;
        let offset = offset_for_position(&self.last_layout, point - origin, window.line_height());
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
        let theme = Theme::of(cx);
        let mut key_context = gpui::KeyContext::default();
        key_context.add(KEY_CONTEXT);
        if self.shape.is_multiline() {
            key_context.add(MULTILINE_KEY_CONTEXT);
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
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_scroll_wheel(cx.listener(Self::on_scroll_wheel))
            .w_full()
            .px(px(10.0))
            .py(px(7.0))
            .rounded(px(8.0))
            .bg(theme.input_bg)
            .border_1()
            .border_color(if self.focus_handle.is_focused(_window) {
                theme.caret
            } else {
                theme.border
            })
            .text_size(px(13.0))
            .line_height(px(18.0))
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
        let line_height = window.line_height();
        let field = self.field.read(cx);
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
        // The IME composition range is underlined so the user can see what is
        // still provisional.
        let runs = if let Some(marked) = marked_range.as_ref() {
            vec![
                TextRun {
                    len: marked.start,
                    ..run.clone()
                },
                TextRun {
                    len: marked.end - marked.start,
                    underline: Some(UnderlineStyle {
                        color: Some(run.color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: text.len() - marked.end,
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect()
        } else {
            vec![run]
        };

        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
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

        let (selection, cursor) = if selected_range.is_empty() {
            let at = position_for_offset(&lines, cursor, line_height).unwrap_or_default();
            (
                Vec::new(),
                Some(fill(
                    Bounds::new(origin + at, gpui::size(CARET_WIDTH, line_height)),
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
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.field.clone()),
            cx,
        );
        let line_height = window.line_height();
        let lines = std::mem::take(&mut prepaint.lines);
        let selection = std::mem::take(&mut prepaint.selection);
        let cursor = prepaint.cursor.take();
        let origin = prepaint.origin;

        // Scrolled text runs past the box in both directions, so everything the
        // field draws is masked to it — text, selection and caret alike.
        window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
            for selection in selection {
                window.paint_quad(selection);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A regional-indicator pair is one grapheme of 8 bytes: stepping by
    /// `char` would land inside it and split the flag in half.
    #[test]
    fn boundaries_step_over_a_flag_emoji() {
        let text = "a🇯🇵b";
        assert_eq!(next_boundary(text, 0), 1, "past 'a'");
        assert_eq!(next_boundary(text, 1), 9, "over the whole flag");
        assert_eq!(previous_boundary(text, 9), 1, "back to the flag's start");
        assert_eq!(previous_boundary(text, 1), 0);
    }

    #[test]
    fn boundaries_step_over_a_combining_mark() {
        // "e" + U+0301 combining acute = one grapheme, three bytes.
        let text = "e\u{301}x";
        assert_eq!(next_boundary(text, 0), 3, "over e+accent together");
        assert_eq!(previous_boundary(text, 3), 0);
    }

    #[test]
    fn boundaries_clamp_at_both_ends() {
        let text = "hi";
        assert_eq!(previous_boundary(text, 0), 0, "no underflow");
        assert_eq!(next_boundary(text, text.len()), text.len(), "no overflow");
        assert_eq!(next_boundary("", 0), 0, "empty is inert");
        assert_eq!(previous_boundary("", 0), 0);
    }

    /// The IME addresses text in UTF-16, so every mapping has to survive
    /// astral-plane characters (2 UTF-16 units, 4 bytes) and CJK (1 unit,
    /// 3 bytes).
    #[test]
    fn utf16_offsets_round_trip() {
        for text in ["ascii", "日本語", "a😀b", "🇯🇵x", "e\u{301}"] {
            let mut byte = 0;
            for ch in text.chars() {
                let utf16 = offset_to_utf16(text, byte);
                assert_eq!(
                    offset_from_utf16(text, utf16),
                    byte,
                    "{text:?} byte {byte} → utf16 {utf16} → back"
                );
                byte += ch.len_utf8();
            }
            assert_eq!(offset_to_utf16(text, byte), text.encode_utf16().count());
        }
    }

    /// A run of typing is one undo step, so `cmd-z` gives back the word rather
    /// than the letter.
    #[test]
    fn typing_a_run_stays_one_undo_group() {
        let mut last = None;
        for offset in 1..6 {
            // Each insert lands exactly where the previous left the caret.
            assert!(
                joins_group(last, EditKind::Insert, offset - 1) || last.is_none(),
                "insert at {offset} should continue the run"
            );
            last = Some((EditKind::Insert, offset));
        }
    }

    /// The first edit has nothing to join.
    #[test]
    fn the_first_edit_opens_a_group() {
        assert!(!joins_group(None, EditKind::Insert, 0));
        assert!(!joins_group(None, EditKind::Delete, 7));
    }

    /// Moving the caret and typing somewhere else is a separate thought, and a
    /// separate undo step.
    #[test]
    fn an_edit_elsewhere_starts_a_new_group() {
        let last = Some((EditKind::Insert, 5));
        assert!(
            joins_group(last, EditKind::Insert, 5),
            "same spot continues"
        );
        assert!(!joins_group(last, EditKind::Insert, 9), "moved away");
        assert!(!joins_group(last, EditKind::Insert, 4), "even by one");
    }

    /// Switching from typing to deleting breaks the group even without moving —
    /// otherwise `cmd-z` after a backspace would give back the typing too.
    #[test]
    fn changing_edit_kind_starts_a_new_group() {
        assert!(!joins_group(
            Some((EditKind::Insert, 5)),
            EditKind::Delete,
            5
        ));
        assert!(!joins_group(
            Some((EditKind::Delete, 5)),
            EditKind::Insert,
            5
        ));
    }

    /// A `\r` that survived would shape as a glyph — `shape_text` splits on
    /// `\n` alone — and put every offset after it out by one.
    #[test]
    fn normalize_folds_crlf_whatever_the_shape() {
        for shape in [Shape::Line, Shape::Rows(3), Shape::Grow { min: 1, max: 4 }] {
            let out = normalize("a\r\nb\rc", shape);
            assert!(!out.contains('\r'), "{shape:?} left a carriage return");
            assert_eq!(out.len(), 5, "{shape:?} changed the character count");
        }
    }

    /// The invariant a single-line field rests on: no newline, ever, however it
    /// got there — and the pasted text is kept, not truncated at the break.
    #[test]
    fn normalize_keeps_a_single_line_single() {
        assert_eq!(normalize("a\nb", Shape::Line), "a b");
        assert_eq!(normalize("a\r\nb", Shape::Line), "a b");
        assert_eq!(normalize("one\ntwo\nthree", Shape::Line), "one two three");
    }

    #[test]
    fn normalize_keeps_breaks_when_the_shape_has_room() {
        assert_eq!(normalize("a\nb", Shape::Rows(2)), "a\nb");
        assert_eq!(normalize("a\r\nb", Shape::Grow { min: 1, max: 3 }), "a\nb");
    }

    /// On a field that cannot hold a newline these collapse to the ends of the
    /// content — which is what they did before logical lines existed, so a
    /// single-line field is untouched by the change.
    #[test]
    fn line_bounds_on_one_line_are_the_whole_content() {
        let text = "the quick brown";
        for offset in [0, 4, text.len()] {
            assert_eq!(line_start(text, offset), 0);
            assert_eq!(line_end(text, offset), text.len());
        }
        assert_eq!(line_start("", 0), 0);
        assert_eq!(line_end("", 0), 0);
    }

    /// `ctrl-a`/`ctrl-e` stop at the newline, not at the ends of the buffer.
    #[test]
    fn line_bounds_are_bounded_by_newlines() {
        //           0123 4567 89
        let text = "one\ntwo\nup";
        assert_eq!(line_start(text, 0), 0, "first line starts at 0");
        assert_eq!(line_end(text, 0), 3, "and ends before the newline");
        assert_eq!(line_start(text, 5), 4, "past the newline, not on it");
        assert_eq!(line_end(text, 5), 7);
        assert_eq!(line_end(text, 8), text.len(), "last line runs to the end");
    }

    /// The cursor sitting on a newline belongs to the line that ends there —
    /// `ctrl-e` must not jump it forward into the next one.
    #[test]
    fn line_bounds_at_a_newline_stay_put() {
        let text = "one\ntwo";
        assert_eq!(line_end(text, 3), 3, "already at the end, so no move");
        assert_eq!(line_start(text, 3), 0);
        assert_eq!(line_start(text, 4), 4, "start of the next line is itself");
    }

    /// Empty lines are lines: a run of newlines must not collapse.
    #[test]
    fn line_bounds_handle_empty_lines() {
        let text = "a\n\nb";
        assert_eq!(line_start(text, 2), 2, "the empty line between them");
        assert_eq!(line_end(text, 2), 2);
    }

    /// option-left lands on the start of the word you were in or just past,
    /// option-right on the end of the next one.
    #[test]
    fn word_motion_walks_word_starts_and_ends() {
        let text = "the quick brown";
        assert_eq!(next_word_boundary(text, 0), 3, "end of 'the'");
        assert_eq!(
            next_word_boundary(text, 3),
            9,
            "skips the space, ends 'quick'"
        );
        assert_eq!(next_word_boundary(text, 6), 9, "from mid-word to its end");
        assert_eq!(previous_word_boundary(text, 15), 10, "start of 'brown'");
        assert_eq!(previous_word_boundary(text, 10), 4, "start of 'quick'");
        assert_eq!(
            previous_word_boundary(text, 6),
            4,
            "from mid-word to its start"
        );
    }

    /// Which punctuation splits a word is UAX#29's call, not ours, and the
    /// answer is useful: identifiers stay whole, paths come apart.
    #[test]
    fn word_motion_keeps_identifiers_whole_but_splits_paths() {
        // A dot or underscore between letters does NOT break a word, so an
        // identifier is one motion.
        for identifier in ["foo.bar", "foo_bar"] {
            assert_eq!(
                next_word_boundary(identifier, 0),
                identifier.len(),
                "{identifier:?} is one word"
            );
            assert_eq!(previous_word_boundary(identifier, identifier.len()), 0);
        }

        // Hyphens and slashes do break.
        let text = "path/to/file";
        assert_eq!(next_word_boundary(text, 0), 4, "stops at the slash");
        assert_eq!(next_word_boundary(text, 4), 7, "then 'to'");
        assert_eq!(
            previous_word_boundary(text, text.len()),
            8,
            "back to 'file'"
        );
        assert_eq!(next_word_boundary("a-b", 0), 1, "hyphen breaks");
    }

    #[test]
    fn word_motion_clamps_and_survives_runs_of_separators() {
        let text = "  a   b  ";
        assert_eq!(previous_word_boundary(text, 0), 0, "no underflow");
        assert_eq!(
            next_word_boundary(text, text.len()),
            text.len(),
            "no overflow"
        );
        assert_eq!(next_word_boundary(text, 0), 3, "over leading spaces to 'a'");
        assert_eq!(previous_word_boundary(text, text.len()), 6, "back to 'b'");
        // Nothing but separators: motion collapses to the ends, never panics.
        assert_eq!(next_word_boundary("   ", 0), 3);
        assert_eq!(previous_word_boundary("   ", 3), 0);
        assert_eq!(next_word_boundary("", 0), 0);
    }

    /// Word motion must not split a grapheme or land mid-character.
    #[test]
    fn word_motion_lands_on_char_boundaries() {
        for text in ["日本語 の テスト", "a😀b c", "🇯🇵 x"] {
            for offset in 0..=text.len() {
                if !text.is_char_boundary(offset) {
                    continue;
                }
                let prev = previous_word_boundary(text, offset);
                let next = next_word_boundary(text, offset);
                assert!(text.is_char_boundary(prev), "{text:?} prev {prev}");
                assert!(text.is_char_boundary(next), "{text:?} next {next}");
                assert!(prev <= offset, "prev never moves forward");
                assert!(next >= offset, "next never moves back");
            }
        }
    }

    #[test]
    fn utf16_offsets_count_surrogate_pairs_as_two() {
        // "😀" is one char, 4 bytes, but TWO UTF-16 code units.
        assert_eq!(offset_to_utf16("😀", 4), 2);
        assert_eq!(offset_from_utf16("😀", 2), 4);
        // CJK: 3 bytes, one unit.
        assert_eq!(offset_to_utf16("日", 3), 1);
        assert_eq!(offset_from_utf16("日", 1), 3);
    }
}
