//! Typing, deletion, block splits, indent and text size.

use super::*;

impl Editor {
    /// Replace whatever is selected with `text`, applying a markdown prefix if
    /// one completes.
    ///
    /// Typing, backspace, delete and IME all land here, so none of them has to
    /// ask whether a selection was empty.
    pub(super) fn insert(&mut self, text: &str, cx: &mut Context<Self>) {
        let painter = Painter::of(cx);
        self.edit(EditKind::Insert, cx, |this| {
            let mut typed = Text::plain(text);
            // A stored mark applies to what is typed next and to nothing else,
            // so it is spent here.
            for mark in this.stored.drain(..) {
                typed.marks.push(markdown::MarkSpan {
                    range: 0..typed.text.len(),
                    mark,
                });
            }
            let splice = this.doc.replace(this.selection, typed);
            this.selection = Selection::at(splice.caret.clamp(&this.doc));
            let shortcut = this.apply_shortcut();
            let promoted = this.promote_quote_marker();
            let inline = this.apply_inline_rule();
            this.track_slash(text, painter);
            std::iter::once(Delta::Spliced(splice))
                .chain(shortcut)
                .chain(promoted)
                .chain(inline)
                .collect()
        });
    }

    /// Open the menu on a typed `/`, and keep its query in step afterwards.
    ///
    /// The query is the text between the `/` and the caret, so there is no
    /// second field and no focus to hand over — typing filters because typing
    /// is what it already was.
    pub(super) fn track_slash(&mut self, typed: &str, painter: Painter) {
        if !self.chrome.slash {
            return;
        }
        let at = self.cursor();
        let text = self
            .doc
            .blocks
            .get(at.block)
            .and_then(|block| block.text_at(at.part))
            .map(|text| text.text.clone())
            .unwrap_or_default();

        if self.slash.is_none() {
            // Only a `/` that starts a word — a URL's slashes are not commands.
            let opened = at.offset.checked_sub(1).filter(|_| typed == "/");
            let starts_word = opened.is_none_or(|slash| {
                text[..slash]
                    .chars()
                    .next_back()
                    .is_none_or(char::is_whitespace)
            });
            // Only in a body: a fence holds its slash literally, and a caption
            // belongs to a block that is already what it is.
            if let Some(slash) = opened.filter(|_| starts_word && at.part == Part::Body) {
                self.slash = Some(Slash::open(
                    Cursor {
                        offset: slash,
                        ..at
                    },
                    painter,
                ));
            }
            return;
        }

        // Anything that leaves the run — a space, a click away, backspacing
        // onto the slash — closes it.
        let Some(query) = self.slash.as_ref().and_then(|slash| slash.query(at, &text)) else {
            self.slash = None;
            return;
        };
        if let Some(slash) = &mut self.slash {
            slash.refilter(&query);
        }
    }

    /// Take the highlighted block, replacing the `/query` that summoned it.
    /// Take `kind`, or the highlighted row when the caller names none — Enter
    /// and a click are the same operation with a different source.
    pub(super) fn confirm_slash(
        &mut self,
        kind: Option<BlockKind>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(slash) = &self.slash else {
            return false;
        };
        let (at, kind) = (slash.at, kind.or_else(|| slash.choice()));
        self.slash = None;
        let Some(kind) = kind else {
            return false;
        };
        let caret = self.cursor();
        self.edit(EditKind::Structure, cx, |this| {
            this.doc
                .edit_at(at, |text| text.remove(at.offset..caret.offset));
            this.doc.set_kind(at.block, kind);
            this.selection =
                Selection::at(Cursor::new(at.block, Part::Body, at.offset).clamp(&this.doc));
            vec![Delta::Spliced(Splice {
                removed: Selection::new(at, caret),
                caret: at,
                blocks: 0,
            })]
        });
        true
    }

    /// Collapse `**bold**` into a mark when its closing delimiter is typed.
    ///
    /// Runs after the insertion, on the text as it now stands, so a paste and a
    /// keystroke reach it the same way.
    pub(super) fn apply_inline_rule(&mut self) -> Vec<Delta> {
        let at = self.cursor();
        // Code is literal to its closing fence, and a caption holds no mark a
        // `![...]` could spell.
        if matches!(at.part, Part::Code | Part::Caption) {
            return Vec::new();
        }
        let Some(text) = self
            .doc
            .blocks
            .get(at.block)
            .and_then(|block| block.text_at(at.part))
        else {
            return Vec::new();
        };
        let Some((open, inner, mark)) = edit::inline_rule(&text.text, at.offset) else {
            return Vec::new();
        };
        let width = open.len();
        let (opening, closing) = (open.clone(), inner.end..at.offset);
        self.doc.edit_at(at, |text| {
            // The closing delimiter first — taking the opening one would move
            // every offset after it.
            text.remove(inner.end..at.offset);
            text.remove(open);
            text.toggle(inner.start - width..inner.end - width, mark);
        });
        self.selection =
            Selection::at(Cursor::new(at.block, at.part, at.offset - 2 * width).clamp(&self.doc));
        // In the order the two removals went. The opening delimiter is ahead of
        // the closing one, so taking that one first left its offsets standing.
        vec![Self::taken(at, closing), Self::taken(at, opening)]
    }

    /// Add `mark` over the selection, or take it away if the whole selection
    /// already carries it. Public because a toolbar reaches the same operation
    /// the key does.
    pub fn toggle_mark(&mut self, mark: Mark, cx: &mut Context<Self>) {
        // Nothing in the source is a mark: the markup is already spelled out,
        // and cmd-B over `**bold**` would fence what it reads.
        if !self.blocks() {
            return;
        }
        // A caret inside a fence is enough to leave one, so this is the mark
        // that does not wait for a range: nothing typed into code is markup,
        // which leaves a stored mark there nothing to mean.
        let leaving_code = matches!(mark, Mark::Code) && self.cursor().part == Part::Code;
        // With nothing selected there is no range to mark, so the mark waits
        // for the next character — ProseMirror's stored marks, and the only way
        // cmd-B before typing can mean anything.
        if self.selection.is_collapsed() && !leaving_code {
            match self.stored.iter().position(|stored| *stored == mark) {
                Some(ix) => drop(self.stored.remove(ix)),
                None => self.stored.push(mark),
            }
            return cx.notify();
        }
        let selection = self.selection;
        self.edit(EditKind::Structure, cx, |this| {
            // Code over more than one line is a fence, which is the only shape
            // markdown has for it, and the same key is the way back out.
            let before = this.doc.blocks.len();
            // Fencing and unfencing rebuild the blocks the selection covered,
            // so what sat inside it has no position left to keep.
            let refenced = |doc: &Doc, head: Cursor| {
                vec![Delta::Spliced(Splice {
                    removed: selection,
                    caret: head,
                    blocks: doc.blocks.len() as isize - before as isize,
                })]
            };
            if matches!(mark, Mark::Code) {
                if let Some(head) = this.doc.unfence(selection) {
                    this.selection = Selection::at(head.clamp(&this.doc));
                    return refenced(&this.doc, head);
                }
                if fenceable(&this.doc, selection) {
                    let head = this.doc.fence(selection);
                    this.selection = Selection::at(head.clamp(&this.doc));
                    return refenced(&this.doc, head);
                }
            }
            // A mark is paint over text that does not move.
            this.doc.toggle_mark(selection, mark);
            vec![]
        });
    }

    /// Turn a typed prefix into the block it spells — `## ` into a heading.
    ///
    /// Runs after every insertion rather than only on space, because the
    /// vocabulary includes prefixes that end in one (`- [ ] `) and prefixes
    /// that do not (```` ``` ````).
    pub(super) fn apply_shortcut(&mut self) -> Option<Delta> {
        let at = self.cursor();
        // A prefix is block syntax; inside a code fence or a table cell it is
        // the literal text the author typed.
        if at.part != Part::Body {
            return None;
        }
        let text = self.doc.blocks.get(at.block)?.text_at(Part::Body)?;
        // Only from the very start of a block, and only up to the caret: a
        // `- ` typed in the middle of a sentence is a hyphen.
        let (hit, len) = shortcut(&text.text)?;
        if at.offset < len {
            return None;
        }
        // Strip the prefix, then turn the block — the same two steps the slash
        // menu takes, so a `## ` and a menu pick land in one place.
        self.doc.edit_at(at, |text| text.remove(0..len));
        self.doc.set_kind(at.block, hit.apply(Text::default()));
        self.selection =
            Selection::at(Cursor::new(at.block, Part::Body, at.offset - len).clamp(&self.doc));
        // The transformation is its own step: undo after typing `## Title`
        // should give back the heading, not the paragraph before the hashes.
        self.history.interrupt();
        Some(Self::taken(at, 0..len))
    }

    /// Promote a quote whose first line is a GFM alert marker into the alert
    /// block kind the parser would have produced from the same markdown.
    ///
    /// The marker line goes the way a block shortcut's prefix does, so what was
    /// written under it stays as the alert's body.
    pub(super) fn promote_quote_marker(&mut self) -> Option<Delta> {
        let at = self.cursor();
        if at.part != Part::Body {
            return None;
        }
        let block = self.doc.blocks.get(at.block)?;
        if !matches!(block.kind, BlockKind::Quote { kind: None, .. }) {
            return None;
        }
        let text = &block.text_at(Part::Body)?.text;
        let first = text.split('\n').next().unwrap_or_default();
        let kind = Self::quote_marker(first)?;
        // The marker line, and the break after it where a body follows.
        let len = first.len() + usize::from(text.len() > first.len());
        self.doc.edit_at(at, |text| text.remove(0..len));
        self.doc.set_kind(
            at.block,
            BlockKind::Quote {
                kind: Some(kind),
                text: Text::default(),
            },
        );
        self.selection = Selection::at(
            Cursor::new(at.block, Part::Body, at.offset.saturating_sub(len)).clamp(&self.doc),
        );
        self.history.interrupt();
        Some(Self::taken(at, 0..len))
    }

    /// The delta for `range` taken out of the text `at` is in — what an anchor
    /// sitting in that text has to move through.
    pub(super) fn taken(at: Cursor, range: Range<usize>) -> Delta {
        let start = Cursor::new(at.block, at.part, range.start);
        Delta::Spliced(Splice {
            removed: Selection {
                anchor: start,
                head: Cursor::new(at.block, at.part, range.end),
            },
            caret: start,
            blocks: 0,
        })
    }

    pub(super) fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        self.delete_back(cx);
    }

    pub(super) fn quote_marker(text: &str) -> Option<markdown::QuoteKind> {
        [
            markdown::QuoteKind::Note,
            markdown::QuoteKind::Tip,
            markdown::QuoteKind::Important,
            markdown::QuoteKind::Warning,
            markdown::QuoteKind::Caution,
        ]
        .into_iter()
        .find(|kind| text.eq_ignore_ascii_case(kind.marker()))
    }

    /// Delete backwards: the selection if there is one, otherwise the character
    /// before the caret, otherwise whatever the start of a block means.
    ///
    /// The kill chords land here too when they have nothing left to take within
    /// the block, so reaching out of one is decided in a single place.
    pub(super) fn delete_back(&mut self, cx: &mut Context<Self>) {
        let at = self.cursor();
        // Reaching out of a block is structural; taking a character is not.
        let kind = if self.selection.is_collapsed() && at.offset == 0 {
            EditKind::Structure
        } else {
            EditKind::Delete
        };
        let painter = Painter::of(cx);
        self.edit(kind, cx, |this| {
            let before = this.doc.blocks.len();
            let splice = if !this.selection.is_collapsed() {
                this.doc.replace(this.selection, Text::default())
            } else if at.offset > 0 {
                this.doc
                    .replace(Selection::new(at.left(&this.doc), at), Text::default())
            } else {
                // `merge_back` outdents, unmarkers, unfences or merges —
                // whichever the block's state calls for — and says where the
                // caret landed.
                match this.doc.merge_back(at) {
                    // The seam between where the caret was and where it landed
                    // is exactly what the merge closed up.
                    Some(head) => Splice {
                        removed: Selection::new(head, at),
                        caret: head,
                        blocks: this.doc.blocks.len() as isize - before as isize,
                    },
                    None => return vec![],
                }
            };
            let head = splice.caret;
            this.selection = Selection::at(head.clamp(&this.doc));
            // Deleting narrows the query too, and backspacing onto the slash
            // itself is what closes the menu.
            this.track_slash("", painter);
            vec![Delta::Spliced(splice)]
        });
    }

    pub(super) fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        self.delete_forward(cx);
    }

    /// Delete forwards, joining the next block when the caret is at the end of
    /// this one — which is what a kill to the end of a line does there too.
    pub(super) fn delete_forward(&mut self, cx: &mut Context<Self>) {
        self.edit(EditKind::Delete, cx, |this| {
            let at = this.cursor();
            let range = if this.selection.is_collapsed() {
                Selection::new(at, at.right(&this.doc))
            } else {
                this.selection
            };
            let splice = this.doc.replace(range, Text::default());
            this.selection = Selection::at(splice.caret.clamp(&this.doc));
            vec![Delta::Spliced(splice)]
        });
    }

    /// Whether an open menu answered Enter itself.
    ///
    /// Every Enter chord asks first, or picking a block would also edit the one
    /// it is turning — and a chord the menu never sees leaves it open over a
    /// query the caret has walked away from.
    pub(super) fn menu_took_enter(&mut self, cx: &mut Context<Self>) -> bool {
        if let Some(choice) = self.pasted.as_ref().map(link::Paste::choice) {
            self.confirm_paste(choice, cx);
            return true;
        }
        self.confirm_slash(None, cx)
    }

    /// Enter. In a body it splits the block; in a code fence it is a newline,
    /// which is the whole reason a fence is worth typing into.
    pub(super) fn split_block(
        &mut self,
        _: &SplitBlock,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.menu_took_enter(cx) {
            return;
        }
        let at = self.cursor();
        match at.part {
            Part::Code => return self.insert("\n", cx),
            // A cell is one line by definition; Enter has nowhere to put a
            // break, so it does nothing rather than something surprising.
            Part::Cell { .. } => return,
            // An image with nothing to show yet is missing one thing, so Enter
            // asks for it rather than carrying on past a blank.
            Part::Caption
                if matches!(
                    self.doc.blocks.get(at.block).map(|block| &block.kind),
                    Some(BlockKind::Image { url, .. }) if url.is_empty()
                ) =>
            {
                return self.prompt_for_url(at.block, window, cx);
            }
            // A caption is one line too, but the block it belongs to is the
            // end of something — so Enter carries on underneath the picture,
            // which is [`Doc::split`]'s answer for a block with no body.
            Part::Caption | Part::Body => {}
        }
        self.edit(EditKind::Structure, cx, |this| {
            let mut deltas = Vec::new();
            if !this.selection.is_collapsed() {
                let splice = this.doc.replace(this.selection, Text::default());
                this.selection = Selection::at(splice.caret.clamp(&this.doc));
                deltas.push(Delta::Spliced(splice));
            }
            let at = this.cursor();
            let new = this.doc.split(at.block, at.offset);
            this.selection = Selection::at(Cursor::new(new, Part::Body, 0).clamp(&this.doc));
            // What followed the caret moved into a block of its own, which
            // everything below it now sits under.
            deltas.push(Delta::Spliced(Splice {
                removed: Selection::at(at),
                caret: Cursor::new(new, Part::Body, 0),
                blocks: 1,
            }));
            deltas
        });
    }

    /// Shift+Enter. In prose it keeps the caret in the block and inserts a
    /// literal newline; the places markdown itself keeps to one line still do.
    pub(super) fn soft_break(&mut self, _: &SoftBreak, _: &mut Window, cx: &mut Context<Self>) {
        if self.menu_took_enter(cx) {
            return;
        }
        match self.cursor().part {
            Part::Body | Part::Code => self.insert("\n", cx),
            Part::Caption | Part::Cell { .. } => {}
        }
    }

    /// Ctrl+Enter. Open a plain paragraph after the current block's whole
    /// subtree and leave the current block's text untouched.
    pub(super) fn insert_paragraph(
        &mut self,
        _: &InsertParagraph,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.menu_took_enter(cx) {
            return;
        }
        if !self.blocks() {
            return;
        }
        self.edit(EditKind::Structure, cx, |this| {
            let at = this.cursor().block;
            let insert_at = this.doc.subtree(at).end;
            let indent = this.doc.blocks.get(at).map_or(0, |block| block.indent);
            this.doc.blocks.insert(
                insert_at,
                Block::at(BlockKind::Paragraph(Text::default()), indent),
            );
            this.doc.repair();
            this.selection = Selection::at(Cursor::new(insert_at, Part::Body, 0).clamp(&this.doc));
            vec![Delta::Opened {
                at: insert_at,
                count: 1,
            }]
        });
    }

    pub(super) fn indent(&mut self, _: &Indent, _: &mut Window, cx: &mut Context<Self>) {
        // In the source there is one block to indent and indenting it would be
        // invisible, so tab is what it is in any text editor: two spaces.
        if !self.blocks() {
            return self.insert(INDENT, cx);
        }
        self.edit(EditKind::Structure, cx, |this| {
            this.doc.indent(this.cursor().block);
            vec![]
        });
    }

    pub(super) fn outdent(&mut self, _: &Outdent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.blocks() {
            return self.unindent(cx);
        }
        self.edit(EditKind::Structure, cx, |this| {
            this.doc.outdent(this.cursor().block);
            vec![]
        });
    }

    pub(super) fn increase_text_size(
        &mut self,
        _: &IncreaseTextSize,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.step_text_size(TextSize::of(cx).step, cx);
    }

    pub(super) fn decrease_text_size(
        &mut self,
        _: &DecreaseTextSize,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.step_text_size(-TextSize::of(cx).step, cx);
    }

    pub(super) fn reset_text_size(
        &mut self,
        _: &ResetTextSize,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        text_size::reset_text_size(cx);
    }

    /// Sizing is not an edit: it changes nothing about the document, so it
    /// leaves no undo step and no anchor moves.
    ///
    /// The step is taken against *this* document's size and stored back as the
    /// shared adjustment, so a press at the end of the range banks up nothing
    /// to work back through on the way down.
    pub(super) fn step_text_size(&mut self, by: f32, cx: &mut Context<Self>) {
        let base = self.text_size.unwrap_or_else(theme::base_text_size);
        let next = TextSize::of(cx).clamp(text_size::resolve(self.text_size, cx) + by);
        text_size::set_adjustment(next - base, cx);
    }

    /// Escape closes an open menu, and otherwise collapses a selection — the
    /// things there are to back out of, innermost first.
    pub(super) fn dismiss(&mut self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        if self.pasted.take().is_none() && self.slash.take().is_none() {
            self.selection = Selection::at(self.selection.head);
        }
        cx.notify();
    }
}
