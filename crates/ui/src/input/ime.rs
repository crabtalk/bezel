use super::*;

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
