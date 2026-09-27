//! The caret: where it is, how it moves, its blink, and the edits that move it.

use super::*;

impl Editor {
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
    pub(super) fn settle_handle(&mut self, window: &Window, cx: &mut Context<Self>) {
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

    pub(super) fn reveal_caret(&mut self, window: &mut Window) {
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
        // Left set after a scroll: a caret far off was placed at a guess, and
        // the frame that builds what lies between can move it again.
        if y != offset.y {
            scroll.set_offset(gpui::point(offset.x, y));
            window.request_animation_frame();
        } else {
            self.reveal = false;
        }
    }

    /// A new caret position restarts its blink and ends any vertical run.
    /// Vertical motion records its next goal after moving the caret.
    pub(super) fn caret_moved(&mut self) {
        self.blink = None;
        self.goal = None;
    }

    /// Blink the caret for as long as the document holds focus.
    pub(super) fn start_blink(&mut self, cx: &mut Context<Self>) {
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
    pub(super) fn cursor(&self) -> Cursor {
        self.selection.head
    }

    /// Put the caret somewhere, collapsed.
    pub(super) fn place(&mut self, cursor: Cursor) {
        self.selection = Selection::at(cursor.clamp(&self.doc));
    }

    /// Move the head, extending the selection or collapsing it — the one path
    /// every motion key takes, so shift is a flag rather than a second handler.
    pub(super) fn moved(
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
    pub(super) fn delete_to(
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

    pub(super) fn head_to(&mut self, head: Cursor, extend: bool) {
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
    pub(super) fn edit(
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
    pub(super) fn vertical(&mut self, down: bool, extend: bool, cx: &mut Context<Self>) {
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

    /// Shift-tab in the source: take back up to one [`INDENT`] of the spaces
    /// before the caret, and nothing else — a line that is not indented has
    /// nothing to give.
    pub(super) fn unindent(&mut self, cx: &mut Context<Self>) {
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
    pub(super) fn caret_text(&self) -> Option<&Text> {
        let at = self.cursor();
        self.doc.blocks.get(at.block)?.text_at(at.part)
    }
}
