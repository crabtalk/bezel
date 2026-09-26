//! The key and mouse handlers, clipboard and undo.

use super::*;

impl TextField {
    pub(super) fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx)
        }
    }

    pub(super) fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx)
        }
    }

    pub(super) fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    pub(super) fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    pub(super) fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx)
    }

    pub(super) fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(line_start(&self.content, self.cursor_offset()), cx);
    }

    pub(super) fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(line_end(&self.content, self.cursor_offset()), cx);
    }

    pub(super) fn select_home(&mut self, _: &SelectHome, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(line_start(&self.content, self.cursor_offset()), cx);
    }

    pub(super) fn select_end(&mut self, _: &SelectEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(line_end(&self.content, self.cursor_offset()), cx);
    }

    pub(super) fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(-1, false, cx);
    }

    pub(super) fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(1, false, cx);
    }

    pub(super) fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(-1, true, cx);
    }

    pub(super) fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
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
    pub(super) fn vertical(&mut self, rows: i32, extend: bool, cx: &mut Context<Self>) {
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
    pub(super) fn insert_newline(
        &mut self,
        _: &InsertNewline,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.shape.is_multiline() {
            self.replace_text_in_range(None, "\n", window, cx);
        }
    }

    pub(super) fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(
            previous_word_boundary(&self.content, self.cursor_offset()),
            cx,
        );
    }

    pub(super) fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(next_word_boundary(&self.content, self.cursor_offset()), cx);
    }

    pub(super) fn select_word_left(
        &mut self,
        _: &SelectWordLeft,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(
            previous_word_boundary(&self.content, self.cursor_offset()),
            cx,
        );
    }

    pub(super) fn select_word_right(
        &mut self,
        _: &SelectWordRight,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(next_word_boundary(&self.content, self.cursor_offset()), cx);
    }

    /// Every delete-by-unit action is "extend the selection over the unit, then
    /// replace it" — so a non-empty selection always wins, matching how every
    /// native field behaves.
    pub(super) fn delete_word_left(
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

    pub(super) fn delete_word_right(
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

    pub(super) fn delete_to_line_start(
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

    pub(super) fn delete_to_line_end(
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

    pub(super) fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let prev = self.previous_boundary(self.cursor_offset());
            if self.cursor_offset() == prev {
                return;
            }
            self.select_to(prev, cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    pub(super) fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let next = self.next_boundary(self.cursor_offset());
            if self.cursor_offset() == next {
                return;
            }
            self.select_to(next, cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    pub(super) fn on_mouse_down(
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

    pub(super) fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    /// Scrolling is the one thing that moves the view without moving the caret,
    /// so it deliberately does not set `follow_caret` — the next frame clamps
    /// this, and the caret is left wherever it was.
    pub(super) fn on_scroll_wheel(
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
    pub(super) fn drag_to(
        &mut self,
        position: Point<Pixels>,
        line_height: Pixels,
        cx: &mut Context<Self>,
    ) {
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

    pub(super) fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    pub(super) fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &normalize(&text, self.shape), window, cx);
        }
    }

    pub(super) fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }

    pub(super) fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx)
        }
    }

    pub(super) fn snapshot(&self) -> Snapshot {
        Snapshot {
            content: self.content.clone(),
            selection: self.selected_range.clone(),
            reversed: self.selection_reversed,
        }
    }

    pub(super) fn restore(&mut self, point: Snapshot, cx: &mut Context<Self>) {
        let edit = Edit::between(&self.content, &point.content);
        self.content = point.content;
        self.selected_range = point.selection;
        self.selection_reversed = point.reversed;
        self.marked_range = None;
        // The next edit must not join whatever group was open before.
        self.last_edit = None;
        self.caret_moved();
        self.edited(edit, cx);
        cx.notify();
    }

    /// Record the state before an edit, unless that edit continues the group the
    /// last one opened. Contiguity is the whole rule: the same kind of edit,
    /// starting where the caret was left. Type a run and it is one step; move
    /// the caret, or switch from typing to deleting, and the next one starts a
    /// group of its own.
    pub(super) fn push_undo(&mut self, kind: EditKind, at: usize) {
        let before = (!joins_group(self.last_edit, kind, at)).then(|| self.snapshot());
        self.history.record(before);
    }

    pub(super) fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        let current = self.snapshot();
        if let Some(point) = self.history.undo(|| current) {
            self.restore(point, cx);
        }
    }

    pub(super) fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        let current = self.snapshot();
        if let Some(point) = self.history.redo(|| current) {
            self.restore(point, cx);
        }
    }
}
