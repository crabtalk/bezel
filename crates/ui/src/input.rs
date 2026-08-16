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
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, ShapedLine,
    SharedString, Style, TextRun, UTF16Selection, UnderlineStyle, Window, actions, div, fill,
    prelude::*, px, relative,
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

/// A single-line text field.
pub struct TextField {
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    /// The IME composition range (underlined while composing).
    marked_range: Option<Range<usize>>,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
}

impl TextField {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: "".into(),
            placeholder: "".into(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_bounds: None,
            is_selecting: false,
        }
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
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
        self.move_to(0, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.content.len(), cx);
    }

    fn select_home(&mut self, _: &SelectHome, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(0, cx);
    }

    fn select_end(&mut self, _: &SelectEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.content.len(), cx);
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
            self.select_to(0, cx);
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
            self.select_to(self.content.len(), cx);
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

    fn on_mouse_down(&mut self, event: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.is_selecting = true;
        if event.modifiers.shift {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        } else {
            self.move_to(self.index_for_mouse_position(event.position), cx)
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
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

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }
        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }
        line.closest_index_for_x(position.x - bounds.left())
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
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let last_layout = self.last_layout.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        Some(Bounds::from_corners(
            gpui::point(
                bounds.left() + last_layout.x_for_index(range.start),
                bounds.top(),
            ),
            gpui::point(
                bounds.left() + last_layout.x_for_index(range.end),
                bounds.bottom(),
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let line_point = self.last_bounds?.localize(&point)?;
        let last_layout = self.last_layout.as_ref()?;
        let utf8_index = last_layout.index_for_x(point.x - line_point.x)?;
        Some(self.offset_to_utf16(utf8_index))
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

/// Paints the shaped line plus selection and caret. A custom element because
/// the caret and selection are geometry derived from the shaped line, which
/// only exists after layout.
struct TextFieldElement {
    field: Entity<TextField>,
}

struct FieldPrepaint {
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
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
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
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
        let content = field.content.clone();
        let selected_range = field.selected_range.clone();
        let cursor = field.cursor_offset();
        let style = window.text_style();

        let (display_text, text_color) = if content.is_empty() {
            (field.placeholder.clone(), theme.text_faint)
        } else {
            (content, style.color)
        };

        let run = TextRun {
            len: display_text.len(),
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
                    len: display_text.len() - marked.end,
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
        let line = window
            .text_system()
            .shape_line(display_text, font_size, &runs, None);

        let (selection, cursor) = if selected_range.is_empty() {
            (
                None,
                Some(fill(
                    Bounds::new(
                        gpui::point(bounds.left() + line.x_for_index(cursor), bounds.top()),
                        gpui::size(px(2.), bounds.bottom() - bounds.top()),
                    ),
                    theme.caret,
                )),
            )
        } else {
            (
                Some(fill(
                    Bounds::from_corners(
                        gpui::point(
                            bounds.left() + line.x_for_index(selected_range.start),
                            bounds.top(),
                        ),
                        gpui::point(
                            bounds.left() + line.x_for_index(selected_range.end),
                            bounds.bottom(),
                        ),
                    ),
                    theme.selection,
                )),
                None,
            )
        };

        FieldPrepaint {
            line: Some(line),
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
        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection)
        }
        let line = prepaint.line.take().unwrap();
        line.paint(
            bounds.origin,
            window.line_height(),
            gpui::TextAlign::Left,
            None,
            window,
            cx,
        )
        .ok();

        // The caret only exists while focused — an unfocused field showing one
        // reads as two cursors on screen.
        if focus_handle.is_focused(window)
            && let Some(cursor) = prepaint.cursor.take()
        {
            window.paint_quad(cursor);
        }

        self.field.update(cx, |field, _| {
            field.last_layout = Some(line);
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
