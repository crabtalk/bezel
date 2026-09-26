//! Select all, copy, cut and paste.

use super::*;

impl Editor {
    pub(super) fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.selection = Selection::all(&self.doc);
        self.history.interrupt();
        self.caret_moved();
        cx.notify();
    }

    /// The selection as markdown — what a copy puts on the clipboard, and what
    /// a paste elsewhere reads back. Inside one fence, the code as it stands.
    pub(super) fn selected_source(&self) -> Option<String> {
        if self.selection.is_collapsed() {
            return None;
        }
        if self.in_fence() {
            let (start, end) = self.selection.clamp(&self.doc).ordered();
            let code = self.doc.blocks[start.block].text_at(Part::Code)?;
            return Some(code.text[start.offset..end.offset].to_string());
        }
        Some({
            let mut slice = self.doc.slice(self.selection);
            slice.normalize_with(&self.marks);
            markdown::serialize_with(&slice, &self.marks)
        })
    }

    pub(super) fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(source) = self.selected_source() {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(source));
        }
    }

    pub(super) fn cut(&mut self, _: &Cut, _: &mut Window, cx: &mut Context<Self>) {
        let Some(source) = self.selected_source() else {
            return;
        };
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(source));
        self.edit(EditKind::Structure, cx, |this| {
            let splice = this.doc.replace(this.selection, Text::default());
            this.selection = Selection::at(splice.caret.clamp(&this.doc));
            vec![Delta::Spliced(splice)]
        });
    }

    /// Markdown in, at the caret. A lone paragraph goes in as inline text with
    /// its marks; anything else arrives as blocks.
    pub(super) fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = cx.read_from_clipboard() else {
            return;
        };
        // Source mode included: the whole document is one fence there.
        if self.in_fence() {
            if let Some(text) = item.text() {
                self.paste_literal(&text, cx);
            }
            return;
        }
        // A picture before its text, because a clipboard carrying both is
        // carrying a name for the picture — which is not the picture. A
        // screenshot has a file name beside its bytes, and a file copied in a
        // file manager has its path beside the path itself.
        for entry in item.entries() {
            let placed = match entry {
                gpui::ClipboardEntry::Image(image) => self.paste_image(image, cx),
                gpui::ClipboardEntry::ExternalPaths(paths) => self.paste_paths(paths, cx),
                gpui::ClipboardEntry::String(_) => false,
            };
            if placed {
                return;
            }
        }
        let Some(source) = item.text() else {
            return;
        };
        let url = source.trim();
        if markdown::is_url(url) {
            return self.paste_url(url.to_string(), cx);
        }
        self.edit(EditKind::Structure, cx, |this| {
            let removed = this.selection;
            let before = this.doc.blocks.len();
            let head = this
                .doc
                .splice(removed, markdown::parse_with(&source, &this.marks));
            this.selection = Selection::at(head.clamp(&this.doc));
            vec![Delta::Spliced(Splice {
                removed,
                caret: head,
                blocks: this.doc.blocks.len() as isize - before as isize,
            })]
        });
    }

    /// Whether the selection starts and ends in one fence's code.
    pub(super) fn in_fence(&self) -> bool {
        let (start, end) = self.selection.ordered();
        start.part == Part::Code && end.part == Part::Code && start.block == end.block
    }

    /// Put `text` in place of the selection as it stands, caret after it.
    pub(super) fn paste_literal(&mut self, text: &str, cx: &mut Context<Self>) {
        self.edit(EditKind::Structure, cx, |this| {
            let splice = this.doc.replace(this.selection, Text::plain(text));
            this.selection = Selection::at(splice.caret.clamp(&this.doc));
            vec![Delta::Spliced(splice)]
        });
    }

    /// A URL is never spliced in as a block. It links whatever is selected, or
    /// lands as a link where the caret is — and only when the block it landed
    /// in held nothing else does it also offer to become a card, which is the
    /// one place a card would not eat a sentence.
    pub(super) fn paste_url(&mut self, url: String, cx: &mut Context<Self>) {
        if self.in_fence() {
            return self.paste_literal(&url, cx);
        }
        // The one paste people expect to *not* overwrite what they chose.
        if !self.selection.is_collapsed() {
            return self.toggle_mark(Mark::Link(url), cx);
        }
        // A card needs a block with nothing else in it; a chip needs a body or
        // a cell to sit in. A fence holds its URL literally and offers neither.
        let at = self.cursor();
        let alone = at.part == Part::Body && self.caret_text().is_some_and(Text::is_empty);
        self.edit(EditKind::Structure, cx, |this| {
            let splice = this.doc.replace(this.selection, Text::link(&url));
            this.selection = Selection::at(splice.caret.clamp(&this.doc));
            vec![Delta::Spliced(splice)]
        });
        // A fence holds its URL literally and a caption cannot spell a mark, so
        // neither has a richer form to offer.
        if self.chrome.paste && !matches!(at.part, Part::Code | Part::Caption) {
            self.pasted = Some(link::Paste::open(at, url, alone));
            cx.notify();
        }
    }

    /// Answer the paste menu: leave the link, or turn its block into a card or
    /// the picture it points at.
    pub(super) fn confirm_paste(&mut self, choice: Choice, cx: &mut Context<Self>) {
        let Some(pasted) = self.pasted.take() else {
            return;
        };
        let ix = pasted.at.block;
        let card = |url, form| BlockKind::Bookmark { url, form };
        match choice {
            Choice::Dismiss => cx.notify(),
            // A chip with a line to itself is a block, which is what gives it
            // room for a favicon; inside a sentence it is a mark over the text
            // that is already there, and only the spelling changes.
            Choice::Chip if pasted.alone => self.turn_into(ix, card(pasted.url, Form::Chip), cx),
            Choice::Chip => self.edit(EditKind::Structure, cx, |this| {
                let end = Cursor {
                    offset: pasted.at.offset + pasted.url.len(),
                    ..pasted.at
                };
                let text = Text {
                    text: pasted.url.clone(),
                    marks: vec![markdown::MarkSpan {
                        range: 0..pasted.url.len(),
                        mark: Mark::Mention {
                            url: pasted.url,
                            form: markdown::Form::Chip,
                        },
                    }],
                };
                let splice = this.doc.replace(Selection::new(pasted.at, end), text);
                this.selection = Selection::at(splice.caret.clamp(&this.doc));
                vec![Delta::Spliced(splice)]
            }),
            Choice::Bookmark => self.turn_into(ix, card(pasted.url, Form::Auto), cx),
            Choice::Embed => self.turn_into(ix, card(pasted.url, Form::Embed), cx),
            Choice::Image => self.turn_into(
                ix,
                BlockKind::Image {
                    url: pasted.url,
                    alt: Text::default(),
                    width: None,
                },
                cx,
            ),
        }
    }
}
