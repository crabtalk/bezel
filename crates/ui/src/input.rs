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
    ]
);

/// The key context the field claims; bindings from [`init`] are scoped to it.
pub const KEY_CONTEXT: &str = "TextField";

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

    #[cfg(target_os = "macos")]
    cx.bind_keys([
        KeyBinding::new("cmd-a", SelectAll, ctx),
        KeyBinding::new("cmd-c", Copy, ctx),
        KeyBinding::new("cmd-x", Cut, ctx),
        KeyBinding::new("cmd-v", Paste, ctx),
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
}

impl TextField {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: "".into(),
            placeholder: "".into(),
            shape: Shape::Line,
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: Vec::new(),
            last_bounds: None,
            is_selecting: false,
        }
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
        self.content = content.into();
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
            // Single line: a pasted newline becomes a space rather than
            // silently truncating what the user pasted.
            self.replace_text_in_range(None, &text.replace('\n', " "), window, cx);
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

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        cx.notify()
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>, line_height: Pixels) -> usize {
        if self.content.is_empty() || self.last_layout.is_empty() {
            return 0;
        }
        let Some(bounds) = self.last_bounds.as_ref() else {
            return 0;
        };
        if position.y < bounds.top() {
            return 0;
        }
        offset_for_position(&self.last_layout, position - bounds.origin, line_height)
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
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

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.marked_range.take();
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
        let start = position_for_offset(&self.last_layout, range.start, line_height)?;
        let end = position_for_offset(&self.last_layout, range.end, line_height)?;
        Some(Bounds::from_corners(
            bounds.origin + start,
            bounds.origin + gpui::point(end.x, end.y + line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let bounds = self.last_bounds?;
        let local = bounds.localize(&point)?;
        let offset = offset_for_position(&self.last_layout, local, window.line_height());
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
        div()
            .key_context(KEY_CONTEXT)
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
        let id = window.request_measured_layout(style, move |_known, available, window, _cx| {
            let text_style = window.text_style();
            let font_size = text_style.font_size.to_pixels(window.rem_size());
            let wrap_width = match available.width {
                gpui::AvailableSpace::Definite(width) => Some(width),
                _ => None,
            };
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
        let runs = if let Some(marked) = field.marked_range.as_ref() {
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

        let (selection, cursor) = if selected_range.is_empty() {
            let at = position_for_offset(&lines, cursor, line_height).unwrap_or_default();
            (
                Vec::new(),
                Some(fill(
                    Bounds::new(bounds.origin + at, gpui::size(px(2.), line_height)),
                    theme.caret,
                )),
            )
        } else {
            (
                selection_rows(&lines, &selected_range, line_height)
                    .into_iter()
                    .map(|rect| {
                        fill(
                            Bounds::new(bounds.origin + rect.origin, rect.size),
                            theme.selection,
                        )
                    })
                    .collect(),
                None,
            )
        };

        FieldPrepaint {
            lines,
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
        for selection in prepaint.selection.drain(..) {
            window.paint_quad(selection);
        }

        let line_height = window.line_height();
        let lines = std::mem::take(&mut prepaint.lines);
        let mut top = bounds.origin;
        for line in &lines {
            line.paint(top, line_height, gpui::TextAlign::Left, None, window, cx)
                .ok();
            top.y += line.size(line_height).height;
        }

        // The caret only exists while focused — an unfocused field showing one
        // reads as two cursors on screen.
        if focus_handle.is_focused(window)
            && let Some(cursor) = prepaint.cursor.take()
        {
            window.paint_quad(cursor);
        }

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
