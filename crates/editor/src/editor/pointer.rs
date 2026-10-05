//! Presses, drags and clicks below the last block.

use super::*;

/// How often a drag held past the scroll box's edge scrolls it, and the most
/// one tick moves however far past the pointer is.
const EDGE_SCROLL_TICK: Duration = Duration::from_millis(16);
const EDGE_SCROLL_MAX: f32 = 48.0;

impl Editor {
    /// The task block whose checkbox `at` landed in, in window coordinates.
    ///
    /// The box that painted rather than the marker column: a press in the
    /// gutter beside it is a press on the row, and still places a caret.
    pub(super) fn checkbox_at(&self, at: gpui::Point<gpui::Pixels>) -> Option<usize> {
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
    pub(super) fn append_tail(&mut self, cx: &mut Context<Self>) -> bool {
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
    pub(super) fn in_drag(&self) -> bool {
        self.dragging.is_some()
            || self.lifted.is_some()
            || self.resizing.is_some()
            || self.table_drag.is_some()
    }

    /// Follow a dragged pointer, wherever in the window it is, and scroll the
    /// host's box while a selection or a lifted block is held past its edge.
    pub(super) fn drag_to(&mut self, position: gpui::Point<gpui::Pixels>, cx: &mut Context<Self>) {
        self.drag_at = Some(position);
        if self.edge_scroll.is_none() && self.edge_step() != 0.0 {
            self.edge_scroll = Some(cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(EDGE_SCROLL_TICK).await;
                    let more = this.update(cx, |this, cx| this.edge_scroll_tick(cx));
                    if !more.unwrap_or(false) {
                        return;
                    }
                }
            }));
        }
        self.follow(position, cx);
    }

    /// How far one edge-scroll tick moves the document: negative up, positive
    /// down, zero while the drag is inside the scroll box or nothing is held.
    fn edge_step(&self) -> f32 {
        let (Some(scroll), Some(at)) = (&self.scroll, self.drag_at) else {
            return 0.0;
        };
        if self.dragging.is_none() && self.lifted.is_none() {
            return 0.0;
        }
        let view = scroll.bounds();
        let past = if at.y < view.top() {
            f32::from(at.y - view.top())
        } else if at.y > view.bottom() {
            f32::from(at.y - view.bottom())
        } else {
            return 0.0;
        };
        (past / 2.0).clamp(-EDGE_SCROLL_MAX, EDGE_SCROLL_MAX)
    }

    /// Scroll by one step and follow the held pointer over what scrolled under
    /// it. Answers whether the drag is still past an edge.
    fn edge_scroll_tick(&mut self, cx: &mut Context<Self>) -> bool {
        let step = self.edge_step();
        let (Some(scroll), Some(at)) = (self.scroll.clone(), self.drag_at) else {
            return false;
        };
        if step == 0.0 {
            self.edge_scroll = None;
            return false;
        }
        let offset = scroll.offset();
        let y = (offset.y - gpui::px(step)).clamp(-scroll.max_offset().y, gpui::px(0.0));
        scroll.set_offset(gpui::point(offset.x, y));
        self.follow(at, cx);
        cx.notify();
        true
    }

    /// Move whatever is being dragged to `position`.
    fn follow(&mut self, position: gpui::Point<gpui::Pixels>, cx: &mut Context<Self>) {
        if self.table_drag.is_some() {
            return self.drag_table_to(position, cx);
        }
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
        if let Some((unit, pressed)) = self.dragging.clone()
            && let Some(hit) = self.layouts.hit(position)
        {
            let (anchor, head) = ui::input::drag_selection(pressed, hit.span(unit, &self.doc));
            self.selection = Selection::new(anchor, head).clamp(&self.doc);
            cx.notify();
        }
    }

    pub(super) fn pressed(
        &mut self,
        position: gpui::Point<gpui::Pixels>,
        click_count: usize,
        modifiers: gpui::Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        ui::popover::close_popup(self, cx, |this| &mut this.block_menu);
        ui::popover::close_popup(self, cx, |this| &mut this.language_menu);
        ui::popover::close_popup(self, cx, |this| &mut this.table_menu);
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
        let unit = ui::input::Granularity::of_clicks(click_count);
        // Shift extends from wherever the anchor already is, which is what
        // makes click-then-shift-click a range.
        self.selection = match modifiers.shift {
            true => self.selection.extend_to(hit),
            false => {
                let span = hit.span(unit, &self.doc);
                Selection::new(span.start, span.end)
            }
        }
        .clamp(&self.doc);
        self.dragging = (!modifiers.shift).then(|| {
            let (start, end) = self.selection.ordered();
            (unit, start..end)
        });
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
    pub(super) fn tail_click(
        &mut self,
        at: gpui::Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(last) = self.doc.blocks.len().checked_sub(1) else {
            return false;
        };
        let Some(bounds) = self.layouts.block_bounds(last) else {
            return false;
        };
        at.y > bounds.origin.y + bounds.size.height && self.append_tail(cx)
    }
}
