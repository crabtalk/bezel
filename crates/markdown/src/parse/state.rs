//! The pulldown-cmark event machine that builds a `Doc`.

use super::*;

/// A list item's marker, held until the item's first paragraph arrives.
#[derive(Clone, Copy)]
pub(super) enum Marker {
    Bullet,
    Ordered(u64),
    Task(bool),
}

impl Marker {
    pub(super) fn into_kind(self, text: Text) -> BlockKind {
        match self {
            Self::Bullet => BlockKind::Bullet(text),
            Self::Ordered(number) => BlockKind::Ordered { number, text },
            Self::Task(checked) => BlockKind::Task { checked, text },
        }
    }
}

#[derive(Default)]
pub(super) struct TableBuild {
    align: Vec<Align>,
    header: Vec<Text>,
    rows: Vec<Vec<Text>>,
    row: Vec<Text>,
    in_head: bool,
}

/// An open blockquote, and how many blocks the document held when it opened.
pub(super) struct OpenQuote {
    kind: Option<QuoteKind>,
    at: usize,
}

#[derive(Default)]
pub(super) struct ParseState {
    pub(super) doc: Doc,
    builder: TextBuilder,
    /// One entry per open list; `Some` counts an ordered list's next number.
    lists: Vec<Option<u64>>,
    /// One entry per open blockquote, innermost last.
    quotes: Vec<OpenQuote>,
    pending_marker: Option<Marker>,
    heading: Option<u8>,
    code: Option<(Option<String>, String)>,
    table: Option<TableBuild>,
    /// The event being handled. What a block that owns no inline run — an
    /// empty marker, a rule — is placed from.
    at: Range<usize>,
    /// Where the run being accumulated started: the first event since the last
    /// block was pushed. A paragraph is pushed by whatever *follows* it, so
    /// the event in hand at that point is the next block's, not this one's.
    span: Option<usize>,
    /// Where each block started, in the order they were pushed.
    pub(super) starts: Vec<usize>,
}

impl ParseState {
    /// Indent level for a block that is not a list marker.
    ///
    /// Only list nesting counts. A blockquote decides a block's *kind*, not how
    /// deep it sits — so a code block inside a quote stays at the quote's own
    /// level rather than acquiring an indent that nothing in the serialized
    /// output could reproduce.
    pub(super) fn indent(&self) -> u8 {
        self.lists.len() as u8
    }

    /// The alert kind a quoted block inherits — the innermost open blockquote's.
    pub(super) fn quote_kind(&self) -> Option<QuoteKind> {
        self.quotes.last().and_then(|open| open.kind)
    }

    /// Append a block, clamping its indent so the document invariant holds
    /// (first block at 0, never more than one deeper than its predecessor).
    pub(super) fn push(&mut self, kind: BlockKind, indent: u8) {
        self.starts.push(self.span.take().unwrap_or(self.at.start));
        let max = self.doc.blocks.last().map_or(0, |b| b.indent + 1);
        self.doc.blocks.push(Block {
            kind,
            indent: indent.min(max),
        });
    }

    /// Emit a pending marker as an empty block so a non-paragraph leaf (a code
    /// block, a table) nests *under* its bullet instead of replacing it.
    pub(super) fn flush_marker(&mut self) {
        let Some(marker) = self.pending_marker.take() else {
            return;
        };
        let indent = self.indent().saturating_sub(1);
        self.push(marker.into_kind(Text::default()), indent);
    }

    /// Close any inline content still open as a block.
    ///
    /// A *tight* list item carries no `Paragraph` tags — pulldown-cmark emits
    /// its text directly between `Item` tags — so every block boundary has to
    /// close the run itself rather than waiting for an end tag that never
    /// comes. Table cells are exempt: their builder is per-cell, and closing it
    /// here would push a block out of the middle of a table.
    pub(super) fn flush_inline(&mut self) {
        if self.table.is_none() && !self.builder.is_empty() {
            self.finish_paragraph();
        }
    }

    /// Close the current run of inline content as a block.
    pub(super) fn finish_paragraph(&mut self) {
        let text = self.builder.take();

        // A paragraph that is nothing but one image is an image block — the
        // `![](media://…)`-on-its-own-line shape. Anything else keeps the image
        // inline, where it stays an image rather than decaying to a link.
        if let [
            MarkSpan {
                range,
                mark: Mark::Image(url),
            },
        ] = text.marks.as_slice()
            && range.start == 0
            && range.end == text.text.len()
        {
            let (caption, width) = split_width(&text.text);
            let (url, alt) = (url.clone(), Text::plain(caption.to_string()));
            self.flush_marker();
            let indent = self.indent();
            self.push(BlockKind::Image { url, alt, width }, indent);
            return;
        }

        // A paragraph that is nothing but a mention is a bookmark — the same
        // `<https://x>` that paints as a chip inside a sentence, given a line
        // of its own. A bare URL is what someone types when they mean a link
        // and `[Title](url)` is what a sentence spells, so carding either would
        // leave no way to write a link that stays one — and it is the paste
        // menu's `Dismiss` that has to write that down.
        //
        // A chip promotes too: off the text flow it can be a real element, and
        // that is the only place a favicon has room to sit.
        //
        // The text has to *be* the URL. `[Example Site](url "chip")` alone on a
        // line keeps its title and stays a paragraph, because promoting it
        // would drop words someone wrote — a block shows only what the preview
        // gave it.
        if let [
            MarkSpan {
                range,
                mark: Mark::Mention { url, form },
            },
        ] = text.marks.as_slice()
            && range.start == 0
            && range.end == text.text.len()
            && text.text == *url
            && is_url(url)
        {
            let (url, form) = (url.clone(), *form);
            self.flush_marker();
            let indent = self.indent();
            self.push(BlockKind::Bookmark { url, form }, indent);
            return;
        }

        if !self.quotes.is_empty() {
            // The bullet comes first so the quote reads as its child rather
            // than replacing it.
            self.flush_marker();
            let kind = self.quote_kind();
            let indent = self.indent();
            self.push(BlockKind::Quote { kind, text }, indent);
        } else if let Some(marker) = self.pending_marker.take() {
            let indent = self.indent().saturating_sub(1);
            self.push(marker.into_kind(text), indent);
        } else {
            let indent = self.indent();
            self.push(BlockKind::Paragraph(text), indent);
        }
    }

    pub(super) fn event(&mut self, event: Event<'_>, range: Range<usize>) {
        // An `End` carries the range of the whole element it closes, which for
        // a list or a quote opens well before the block that just went in.
        // Only something that starts content can start a run.
        if !matches!(event, Event::End(_)) {
            self.span.get_or_insert(range.start);
        }
        self.at = range;
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),

            Event::Text(t) => match &mut self.code {
                Some((_, code)) => code.push_str(&t),
                None => self.builder.text.push_str(&t),
            },
            Event::Code(t) => self.builder.wrap(Mark::Code, &t),
            // Raw HTML is content, not structure: this model has no HTML node,
            // so it survives as the literal text the author typed.
            Event::Html(t) | Event::InlineHtml(t) => self.builder.text.push_str(&t),
            // Soft and hard breaks are both just a line break in a block —
            // the distinction has no meaning in this model, or in Notion.
            Event::SoftBreak | Event::HardBreak => match &mut self.code {
                Some((_, code)) => code.push('\n'),
                None => self.builder.text.push('\n'),
            },
            Event::Rule => {
                self.flush_inline();
                self.flush_marker();
                let indent = self.indent();
                self.push(BlockKind::Rule, indent);
            }
            Event::TaskListMarker(checked) => {
                self.pending_marker = Some(Marker::Task(checked));
            }
            Event::FootnoteReference(label) => {
                self.builder.text.push_str(&format!("[^{label}]"));
            }
            _ => {}
        }
    }

    pub(super) fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Heading { level, .. } => {
                self.flush_inline();
                self.heading = Some(level as u8);
            }
            Tag::BlockQuote(kind) => {
                self.flush_inline();
                let at = self.doc.blocks.len();
                self.quotes.push(OpenQuote {
                    kind: kind.map(QuoteKind::from),
                    at,
                });
            }
            Tag::CodeBlock(kind) => {
                self.flush_inline();
                self.flush_marker();
                let language = match kind {
                    CodeBlockKind::Fenced(info) => {
                        let tag = info.split_whitespace().next().unwrap_or("");
                        (!tag.is_empty()).then(|| tag.to_string())
                    }
                    CodeBlockKind::Indented => None,
                };
                self.code = Some((language, String::new()));
            }
            Tag::List(start) => {
                self.flush_inline();
                // An item whose content is only a nested list still has to emit
                // its own marker first. `flush_inline` covers the item that had
                // text; this covers the empty one, whose pending marker the
                // nested `Start(Item)` would otherwise overwrite — losing a
                // level of nesting. It runs before the push so the marker is
                // numbered at the outer list's depth.
                self.flush_marker();
                self.lists.push(start);
            }
            Tag::Item => {
                self.flush_inline();
                self.pending_marker = Some(match self.lists.last_mut() {
                    Some(Some(number)) => {
                        let n = *number;
                        *number += 1;
                        Marker::Ordered(n)
                    }
                    _ => Marker::Bullet,
                });
            }
            Tag::Table(aligns) => {
                self.flush_inline();
                self.flush_marker();
                self.table = Some(TableBuild {
                    align: aligns.iter().map(align_of).collect(),
                    ..TableBuild::default()
                });
            }
            Tag::TableHead => {
                if let Some(table) = &mut self.table {
                    table.in_head = true;
                }
            }
            Tag::Emphasis => {
                self.builder.open(Mark::Italic);
            }
            Tag::Strong => {
                self.builder.open(Mark::Bold);
            }
            Tag::Strikethrough => {
                self.builder.open(Mark::Strike);
            }
            // A rich link is its own mark rather than a flag on a link: where
            // the spelling came from is what decides the painting, and a flag
            // beside the mark is a second place for that to be recorded.
            Tag::Link {
                link_type,
                dest_url,
                title,
                ..
            } => {
                let url = dest_url.into_string();
                let form = match link_type {
                    LinkType::Autolink => Some(Form::Auto),
                    _ => Form::from_title(&title),
                };
                self.builder.open(match form {
                    Some(form) => Mark::Mention { url, form },
                    None => Mark::Link(url),
                });
            }
            Tag::Image { dest_url, .. } => {
                self.builder.open(Mark::Image(dest_url.into_string()));
            }
            _ => {}
        }
    }

    pub(super) fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::HtmlBlock => self.flush_inline(),
            TagEnd::Heading(_) => {
                self.flush_marker();
                let level = self.heading.take().unwrap_or(1);
                let mut text = self.builder.take();
                collapse_to_one_line(&mut text);
                let indent = self.indent();
                self.push(BlockKind::Heading { level, text }, indent);
            }
            // Flushed before the depth changes, so trailing text still lands
            // as a quote rather than as a paragraph after it.
            TagEnd::BlockQuote(_) => {
                self.flush_inline();
                // pulldown-cmark takes the marker line out of the text, so a
                // blockquote that held nothing else arrives here empty.
                if let Some(open) = self.quotes.pop()
                    && open.kind.is_some()
                    && self.doc.blocks.len() == open.at
                {
                    let indent = self.indent();
                    self.push(
                        BlockKind::Quote {
                            kind: open.kind,
                            text: Text::default(),
                        },
                        indent,
                    );
                }
            }
            TagEnd::CodeBlock => {
                if let Some((language, code)) = self.code.take() {
                    let indent = self.indent();
                    // The fence swallows the final newline; storing it would
                    // grow the block by one blank line on every round trip.
                    let code = code.strip_suffix('\n').map_or(code.clone(), str::to_string);
                    self.push(
                        BlockKind::Code {
                            language,
                            code: Text::plain(code),
                        },
                        indent,
                    );
                }
            }
            TagEnd::List(_) => {
                self.flush_inline();
                self.lists.pop();
            }
            // A tight item's text arrives with no `Paragraph` tag to close it,
            // so the item's end is what turns it into the marker block. Only an
            // item that produced nothing at all falls through to an empty one.
            TagEnd::Item => {
                self.flush_inline();
                self.flush_marker();
            }
            TagEnd::Table => {
                if let Some(table) = self.table.take() {
                    let indent = self.indent();
                    self.push(
                        BlockKind::Table {
                            align: table.align,
                            header: table.header,
                            rows: table.rows,
                        },
                        indent,
                    );
                }
            }
            TagEnd::TableHead => {
                if let Some(table) = &mut self.table {
                    table.header = std::mem::take(&mut table.row);
                    table.in_head = false;
                }
            }
            TagEnd::TableRow => {
                if let Some(table) = &mut self.table {
                    let row = std::mem::take(&mut table.row);
                    table.rows.push(row);
                }
            }
            TagEnd::TableCell => {
                let mut cell = self.builder.take();
                collapse_to_one_line(&mut cell);
                if let Some(table) = &mut self.table {
                    table.row.push(cell);
                }
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough | TagEnd::Link => {
                self.builder.close();
            }
            TagEnd::Image => self.builder.close(),
            _ => {}
        }
    }
}

pub(super) fn align_of(alignment: &Alignment) -> Align {
    match alignment {
        Alignment::Center => Align::Center,
        Alignment::Right => Align::Right,
        Alignment::Left | Alignment::None => Align::Left,
    }
}
