//! The platform's text input handler.

use std::ops::Range;

use gpui::{Context, EntityInputHandler, UTF16Selection, Window};
use markdown::{Cursor, Selection};

use crate::editor::Editor;

/// Typed text and IME arrive here. Offsets are within the caret's block, which
/// is the unit the platform is told about — a block is a paragraph's worth of
/// text, so a candidate window never has to be anchored across one.
impl EntityInputHandler for Editor {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let text = self.caret_text()?;
        let range = range.start.min(text.text.len())..range.end.min(text.text.len());
        if range.start != range.end {
            *adjusted = Some(range.clone());
        }
        Some(text.text.get(range)?.to_string())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        // The platform is told about one text at a time, so a selection that
        // leaves the caret's own is reported collapsed — there are no
        // coordinates here in which to express it.
        let (start, end) = self.selection.ordered();
        let spans_one = start.block == end.block && start.part == end.part;
        let head = self.selection.head;
        let range = if spans_one {
            start.offset..end.offset
        } else {
            head.offset..head.offset
        };
        Some(UTF16Selection {
            reversed: spans_one && self.selection.head == start,
            range,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.clone()
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked = None;
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // A whole URL arriving in one insert is a paste whatever delivered it,
        // and on the web it is the only shape one arrives in: the browser's
        // clipboard cannot be read synchronously, so gpui hands the DOM paste
        // event to this handler rather than to the `Paste` action. Typing
        // cannot reach here — a URL typed by hand arrives a character at a
        // time, and none of those characters is a URL.
        if range.is_none() && self.marked.is_none() && markdown::is_url(text.trim()) {
            return self.paste_url(text.trim().to_string(), cx);
        }
        // The platform's range is within the caret's own text, so it becomes a
        // selection there and the insert path does the rest.
        if let Some(range) = range.or_else(|| self.marked.clone()) {
            let at = self.cursor();
            self.selection = Selection::new(
                Cursor {
                    offset: range.start,
                    ..at
                },
                Cursor {
                    offset: range.end,
                    ..at
                },
            );
        }
        self.marked = None;
        self.insert(text, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        marked: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(range) = range.or_else(|| self.marked.clone()) {
            let at = self.cursor();
            self.doc.edit_at(at, |body| body.remove(range.clone()));
            self.place(Cursor {
                offset: range.start,
                ..at
            });
        }
        // Composition runs outside the shortcut path: a half-typed candidate is
        // not a markdown prefix, and turning it into one mid-composition would
        // pull the text out from under the IME.
        let at = self.cursor();
        let start = at.offset;
        self.doc.edit_at(at, |body| body.insert(start, text));
        self.marked = Some(start..start + text.len());
        self.place(Cursor {
            offset: marked
                .map(|range| start + range.end.min(text.len()))
                .unwrap_or(start + text.len()),
            ..at
        });
        cx.notify();
    }

    /// Where a range paints, so a candidate window opens under the text it is
    /// composing rather than at the window's origin.
    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: gpui::Bounds<gpui::Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<gpui::Bounds<gpui::Pixels>> {
        let at = self.cursor();
        let start = Cursor {
            offset: range.start,
            ..at
        };
        let (origin, line_height) = self.layouts.position(start)?;
        let end = self
            .layouts
            .position(Cursor {
                offset: range.end,
                ..at
            })
            .map(|(point, _)| point)
            .filter(|point| point.y == origin.y);
        let width = end.map_or(gpui::px(0.0), |point| point.x - origin.x);
        Some(gpui::Bounds::new(origin, gpui::size(width, line_height)))
    }

    fn character_index_for_point(
        &mut self,
        point: gpui::Point<gpui::Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let hit = self.layouts.hit(point)?;
        let at = self.cursor();
        (hit.block == at.block && hit.part == at.part).then_some(hit.offset)
    }
}
