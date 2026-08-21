//! The editing surface.
//!
//! The document is the single source of truth. There is no `TextField` per
//! block: a selection is a pair of `Cursor`s into one `Doc`, which is what
//! makes Enter split, Backspace merge and Tab indent into list operations
//! rather than negotiations between separate widgets each owning a string.
//!
//! Everything about *what* an edit does lives in `markdown` — `edit` and
//! `select` — and is tested there without a window. This crate owns only what
//! needs one: a focus handle, key bindings, the platform input handler, and
//! turning a click into a position.

use gpui::{
    App, Context, CursorStyle, ElementInputHandler, FocusHandle, Focusable, MouseButton, Render,
    Styled as _, Window, canvas, div, prelude::*,
};
use markdown::{
    BlockKind, BlockLayouts, Cursor, Doc, Form, Mark, Part, Selection, Text, edit, edit::shortcut,
};
use std::ops::Range;
use theme::Theme;

use crate::{
    history::{EditKind, History},
    link::{self, Choice},
    slash::Slash,
};

mod input;
mod keys;
pub(crate) mod menu;

pub use keys::init;
use keys::{
    Backspace, Copy, Cut, Delete, DeleteToHome, DeleteWordLeft, DeleteWordRight, Dismiss, Down,
    DuplicateBlock, End, Home, Indent, KillLine, Left, MoveBlockDown, MoveBlockUp, Outdent, Paste,
    Redo, RemoveBlock, Right, SelectAll, SelectDown, SelectEnd, SelectHome, SelectLeft,
    SelectRight, SelectUp, SelectWordLeft, SelectWordRight, SplitBlock, ToggleBold, ToggleCode,
    ToggleItalic, ToggleStrike, Undo, Up, WordLeft, WordRight,
};

const CONTEXT: &str = "BezelEditor";

/// Shown on the focused block while it is empty — the only discoverable place
/// to say that `/` does anything.
const PLACEHOLDER: &str = "Type / for commands";

/// The handle's box, and how far left of the text it sits. Wide enough to be
/// hit without crowding the margin.
const HANDLE_SIZE: f32 = 18.0;
const HANDLE_GUTTER: f32 = 22.0;

/// Whether a selection is prose covering more than one line — two blocks, or
/// one line break inside a single block.
///
/// What a chord means about a selection is the editor's to decide; a [`Doc`]
/// has no opinion about it. A selection reaching into a table or a fence is not
/// prose and keeps the inline behaviour, because half a table has no lines to
/// make a fence out of.
fn fenceable(doc: &Doc, selection: Selection) -> bool {
    let spans = doc.spans(selection);
    let covered = |at: &Cursor, range: &Range<usize>| {
        doc.blocks[at.block]
            .text_at(at.part)
            .and_then(|text| text.text.get(range.clone()))
    };
    spans.iter().all(|(at, _)| at.part == Part::Body)
        && (spans.len() > 1
            || spans
                .iter()
                .any(|(at, range)| covered(at, range).is_some_and(|text| text.contains('\n'))))
}

pub struct Editor {
    doc: Doc,
    /// Collapsed for an ordinary caret, so there is one position here rather
    /// than a caret and a range that can disagree.
    selection: Selection,
    focus_handle: FocusHandle,
    /// The IME composition range within the caret's text, underlined while it
    /// is being composed.
    marked: Option<Range<usize>>,
    /// Where each text landed last frame, so a click can be turned into a
    /// caret. Only paint knows this, so the renderer fills it.
    layouts: BlockLayouts,
    history: History,
    /// Marks the next typed character will carry — cmd-B at a collapsed caret,
    /// which otherwise has no range to apply to and so would do nothing.
    /// Cleared by any motion, because they belong to a spot and not to a mood.
    stored: Vec<Mark>,
    /// The open slash menu, if `/` started one.
    slash: Option<Slash>,
    /// The open paste menu, if a URL landed in a block of its own.
    pasted: Option<link::Paste>,
    /// The block the pointer is over, which is the only one showing a handle.
    hovered: Option<usize>,
    /// A block being dragged by its handle, and where it would land.
    lifted: Option<(usize, usize)>,
    /// The block menu the handle opened, and where to anchor it.
    block_menu: Option<(usize, gpui::Point<gpui::Pixels>)>,
    /// The language menu a fence's header opened, and the block it belongs to.
    language_menu: Option<(usize, gpui::Point<gpui::Pixels>)>,
    /// Set by the handle's press so the editor's own press does not undo it.
    handle_pressed: bool,
    /// Where the editor's own box starts, so a position recorded in window
    /// coordinates can be placed inside it.
    origin: gpui::Point<gpui::Pixels>,
    /// Whether the pointer is dragging out a selection.
    dragging: bool,
    /// Whether the pointer is over painted text, which is the only place the
    /// editor claims an I-beam.
    over_text: bool,
    /// The host's scroll box, when it gave one, and whether the caret still
    /// owes it a reveal.
    scroll: Option<gpui::ScrollHandle>,
    reveal: bool,
    /// The point vertical motion is trying to keep. Held across a run of
    /// up/down so walking through a short line and out the other side returns
    /// to the column you started in, and dropped by anything horizontal —
    /// which is every other way the caret moves.
    ///
    /// The *row* is held as well as the column because an offset at a soft
    /// wrap belongs to two rows and answers with the first, so a caret that
    /// derived its own row would step down into the same one forever.
    goal: Option<gpui::Point<gpui::Pixels>>,
}

impl Editor {
    pub fn new(source: &str, cx: &mut Context<Self>) -> Self {
        let doc = markdown::parse(source);
        Self {
            // Clamped, not defaulted: a document opening on a fence or a table
            // has no body at block zero, and a caret claiming one resolves
            // against nothing until something moves it.
            selection: Selection::at(Cursor::default().clamp(&doc)),
            doc,
            focus_handle: cx.focus_handle(),
            marked: None,
            layouts: BlockLayouts::default(),
            history: History::default(),
            stored: Vec::new(),
            slash: None,
            pasted: None,
            hovered: None,
            lifted: None,
            block_menu: None,
            language_menu: None,
            handle_pressed: false,
            origin: gpui::Point::default(),
            dragging: false,
            over_text: false,
            scroll: None,
            reveal: false,
            goal: None,
        }
    }

    /// How many undo steps to keep. App-wide configuration would be a gpui
    /// global alongside [`init`], not a `Theme` field — the theme is rebuilt on
    /// every light/dark switch, which would quietly reset anything behavioural
    /// parked in it.
    pub fn with_undo_limit(mut self, limit: usize) -> Self {
        self.history = History::with_limit(limit);
        self
    }

    /// The box the document scrolls in, so typing off the bottom follows the
    /// caret down.
    ///
    /// The host's rather than the editor's: a document goes in whatever pane
    /// the app gives it, and the gutter handle, the drop indicator and the
    /// menus are all placed absolutely against this editor's own origin — put
    /// the scroll box here and every one of them would be offset twice.
    pub fn with_scroll(mut self, handle: gpui::ScrollHandle) -> Self {
        self.scroll = Some(handle);
        self
    }

    /// Bring the caret back into view.
    ///
    /// Read *after* paint, because a block that has only just appeared — the
    /// one Enter made — has no position recorded until it has painted once,
    /// which is exactly the case worth scrolling for.
    fn reveal_caret(&mut self, cx: &mut Context<Self>) {
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
        self.reveal = false;

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
        if y != offset.y {
            scroll.set_offset(gpui::point(offset.x, y));
            cx.notify();
        }
    }

    pub fn doc(&self) -> &Doc {
        &self.doc
    }

    pub fn selection(&self) -> Selection {
        self.selection
    }

    /// Where the selection sits on screen, in window coordinates, so a host can
    /// float a toolbar at it.
    ///
    /// The head's row only — a selection spanning ten blocks wants its bubble
    /// where the pointer left off, not centred over the whole span. `None` when
    /// nothing is selected or the caret has not painted yet.
    pub fn selection_bounds(&self) -> Option<gpui::Bounds<gpui::Pixels>> {
        if self.selection.is_collapsed() {
            return None;
        }
        let (point, line_height) = self.layouts.position(self.selection.head)?;
        Some(gpui::Bounds::new(
            point,
            gpui::size(gpui::px(0.0), line_height),
        ))
    }

    /// Where typing would land — the moving end of the selection.
    fn cursor(&self) -> Cursor {
        self.selection.head
    }

    /// Put the caret somewhere, collapsed.
    fn place(&mut self, cursor: Cursor) {
        self.selection = Selection::at(cursor.clamp(&self.doc));
    }

    /// Move the head, extending the selection or collapsing it — the one path
    /// every motion key takes, so shift is a flag rather than a second handler.
    fn moved(
        &mut self,
        extend: bool,
        to: impl FnOnce(Cursor, &Doc) -> Cursor,
        cx: &mut Context<Self>,
    ) {
        let head = to(self.selection.head, &self.doc).clamp(&self.doc);
        self.head_to(head, extend);
        // Every horizontal motion drops the goal; the two vertical ones put it
        // back after calling this.
        self.goal = None;
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
    fn delete_to(
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
        self.edit(EditKind::Delete, cx, |this| {
            let head = this
                .doc
                .replace(Selection::new(target, at), Text::default());
            this.selection = Selection::at(head.clamp(&this.doc));
            this.track_slash("");
        });
    }

    fn head_to(&mut self, head: Cursor, extend: bool) {
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
    }

    /// Every mutation goes through here, so none of them can forget to record
    /// a step and none of them has to know how steps coalesce.
    fn edit(&mut self, kind: EditKind, cx: &mut Context<Self>, edit: impl FnOnce(&mut Self)) {
        // Any edit answers the paste menu by ignoring it — whatever it offered
        // was about a block that no longer holds only the link.
        self.pasted = None;
        self.history.record(kind, &self.doc, self.selection);
        edit(self);
        self.history.landed(kind, self.selection);
        // Typing moves the caret as surely as an arrow key does, and a split
        // moves it onto a block that does not exist until this frame paints.
        self.reveal = true;
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
    fn vertical(&mut self, down: bool, extend: bool, cx: &mut Context<Self>) {
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
        let from = self.goal.unwrap_or(at);
        match self.layouts.step_row(head, from, down) {
            Some((to, row)) => {
                self.head_to(to.clamp(&self.doc), extend);
                self.goal = Some(gpui::point(from.x, row));
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
                self.goal = Some(from);
            }
        }
        cx.notify();
    }

    /// The document as markdown — normalized, because that is the form that
    /// survives being read back.
    pub fn source(&self) -> String {
        let mut doc = self.doc.clone();
        doc.normalize();
        markdown::serialize(&doc)
    }

    /// Replace whatever is selected with `text`, applying a markdown prefix if
    /// one completes.
    ///
    /// Typing, backspace, delete and IME all land here, so none of them has to
    /// ask whether a selection was empty.
    fn insert(&mut self, text: &str, cx: &mut Context<Self>) {
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
            let head = this.doc.replace(this.selection, typed);
            this.selection = Selection::at(head.clamp(&this.doc));
            this.apply_shortcut();
            this.apply_inline_rule();
            this.track_slash(text);
        });
    }

    /// Open the menu on a typed `/`, and keep its query in step afterwards.
    ///
    /// The query is the text between the `/` and the caret, so there is no
    /// second field and no focus to hand over — typing filters because typing
    /// is what it already was.
    fn track_slash(&mut self, typed: &str) {
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
            if let Some(slash) = opened.filter(|_| starts_word && at.part != Part::Code) {
                self.slash = Some(Slash::open(Cursor {
                    offset: slash,
                    ..at
                }));
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
        });
        true
    }

    /// Collapse `**bold**` into a mark when its closing delimiter is typed.
    ///
    /// Runs after the insertion, on the text as it now stands, so a paste and a
    /// keystroke reach it the same way.
    fn apply_inline_rule(&mut self) {
        let at = self.cursor();
        // Code is literal to its closing fence.
        if at.part == Part::Code {
            return;
        }
        let Some(text) = self
            .doc
            .blocks
            .get(at.block)
            .and_then(|block| block.text_at(at.part))
        else {
            return;
        };
        let Some((open, inner, mark)) = edit::inline_rule(&text.text, at.offset) else {
            return;
        };
        let width = open.len();
        self.doc.edit_at(at, |text| {
            // The closing delimiter first — taking the opening one would move
            // every offset after it.
            text.remove(inner.end..at.offset);
            text.remove(open);
            text.toggle(inner.start - width..inner.end - width, mark);
        });
        self.selection =
            Selection::at(Cursor::new(at.block, at.part, at.offset - 2 * width).clamp(&self.doc));
    }

    /// Add `mark` over the selection, or take it away if the whole selection
    /// already carries it. Public because a toolbar reaches the same operation
    /// the key does.
    pub fn toggle_mark(&mut self, mark: Mark, cx: &mut Context<Self>) {
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
            if matches!(mark, Mark::Code) {
                if let Some(head) = this.doc.unfence(selection) {
                    this.selection = Selection::at(head.clamp(&this.doc));
                    return;
                }
                if fenceable(&this.doc, selection) {
                    let head = this.doc.fence(selection);
                    this.selection = Selection::at(head.clamp(&this.doc));
                    return;
                }
            }
            this.doc.toggle_mark(selection, mark);
        });
    }

    /// Turn a typed prefix into the block it spells — `## ` into a heading.
    ///
    /// Runs after every insertion rather than only on space, because the
    /// vocabulary includes prefixes that end in one (`- [ ] `) and prefixes
    /// that do not (```` ``` ````).
    fn apply_shortcut(&mut self) {
        let at = self.cursor();
        // A prefix is block syntax; inside a code fence or a table cell it is
        // the literal text the author typed.
        if at.part != Part::Body {
            return;
        }
        let Some(block) = self.doc.blocks.get(at.block) else {
            return;
        };
        let Some(text) = block.text_at(Part::Body) else {
            return;
        };
        // Only from the very start of a block, and only up to the caret: a
        // `- ` typed in the middle of a sentence is a hyphen.
        let Some((hit, len)) = shortcut(&text.text) else {
            return;
        };
        if at.offset < len {
            return;
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
    }

    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        self.delete_back(cx);
    }

    /// Delete backwards: the selection if there is one, otherwise the character
    /// before the caret, otherwise whatever the start of a block means.
    ///
    /// The kill chords land here too when they have nothing left to take within
    /// the block, so reaching out of one is decided in a single place.
    fn delete_back(&mut self, cx: &mut Context<Self>) {
        let at = self.cursor();
        // Reaching out of a block is structural; taking a character is not.
        let kind = if self.selection.is_collapsed() && at.offset == 0 {
            EditKind::Structure
        } else {
            EditKind::Delete
        };
        self.edit(kind, cx, |this| {
            let head = if !this.selection.is_collapsed() {
                this.doc.replace(this.selection, Text::default())
            } else if at.offset > 0 {
                this.doc
                    .replace(Selection::new(at.left(&this.doc), at), Text::default())
            } else {
                // `merge_back` outdents, unmarkers, unfences or merges —
                // whichever the block's state calls for — and says where the
                // caret landed.
                match this.doc.merge_back(at) {
                    Some(head) => head,
                    None => return,
                }
            };
            this.selection = Selection::at(head.clamp(&this.doc));
            // Deleting narrows the query too, and backspacing onto the slash
            // itself is what closes the menu.
            this.track_slash("");
        });
    }

    fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        self.delete_forward(cx);
    }

    /// Delete forwards, joining the next block when the caret is at the end of
    /// this one — which is what a kill to the end of a line does there too.
    fn delete_forward(&mut self, cx: &mut Context<Self>) {
        self.edit(EditKind::Delete, cx, |this| {
            let at = this.cursor();
            let range = if this.selection.is_collapsed() {
                Selection::new(at, at.right(&this.doc))
            } else {
                this.selection
            };
            let head = this.doc.replace(range, Text::default());
            this.selection = Selection::at(head.clamp(&this.doc));
        });
    }

    /// Enter. In a body it splits the block; in a code fence it is a newline,
    /// which is the whole reason a fence is worth typing into.
    fn split_block(&mut self, _: &SplitBlock, _: &mut Window, cx: &mut Context<Self>) {
        // A menu owns Enter while it is open, or picking a block would also
        // split the one it is turning.
        if let Some(choice) = self.pasted.as_ref().map(link::Paste::choice) {
            return self.confirm_paste(choice, cx);
        }
        if self.confirm_slash(None, cx) {
            return;
        }
        let at = self.cursor();
        match at.part {
            Part::Code => return self.insert("\n", cx),
            // A cell is one line by definition; Enter has nowhere to put a
            // break, so it does nothing rather than something surprising.
            Part::Cell { .. } => return,
            Part::Body => {}
        }
        self.edit(EditKind::Structure, cx, |this| {
            if !this.selection.is_collapsed() {
                let head = this.doc.replace(this.selection, Text::default());
                this.selection = Selection::at(head.clamp(&this.doc));
            }
            let at = this.cursor();
            let new = this.doc.split(at.block, at.offset);
            this.selection = Selection::at(Cursor::new(new, Part::Body, 0).clamp(&this.doc));
        });
    }

    fn indent(&mut self, _: &Indent, _: &mut Window, cx: &mut Context<Self>) {
        self.edit(EditKind::Structure, cx, |this| {
            this.doc.indent(this.cursor().block);
        });
    }

    fn outdent(&mut self, _: &Outdent, _: &mut Window, cx: &mut Context<Self>) {
        self.edit(EditKind::Structure, cx, |this| {
            this.doc.outdent(this.cursor().block);
        });
    }

    /// Escape closes an open menu, and otherwise collapses a selection — the
    /// things there are to back out of, innermost first.
    fn dismiss(&mut self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        if self.pasted.take().is_none() && self.slash.take().is_none() {
            self.selection = Selection::at(self.selection.head);
        }
        cx.notify();
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.selection = Selection::all(&self.doc);
        self.history.interrupt();
        cx.notify();
    }

    /// The selection as markdown — what a copy puts on the clipboard, and what
    /// a paste elsewhere reads back.
    fn selected_source(&self) -> Option<String> {
        (!self.selection.is_collapsed()).then(|| {
            let mut slice = self.doc.slice(self.selection);
            slice.normalize();
            markdown::serialize(&slice)
        })
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(source) = self.selected_source() {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(source));
        }
    }

    fn cut(&mut self, _: &Cut, _: &mut Window, cx: &mut Context<Self>) {
        let Some(source) = self.selected_source() else {
            return;
        };
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(source));
        self.edit(EditKind::Structure, cx, |this| {
            let head = this.doc.replace(this.selection, Text::default());
            this.selection = Selection::at(head.clamp(&this.doc));
        });
    }

    /// Markdown in, at the caret. A lone paragraph goes in as inline text with
    /// its marks; anything else arrives as blocks.
    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        let Some(source) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        let url = source.trim();
        if markdown::is_url(url) {
            return self.paste_url(url.to_string(), cx);
        }
        self.edit(EditKind::Structure, cx, |this| {
            let head = this.doc.splice(this.selection, markdown::parse(&source));
            this.selection = Selection::at(head.clamp(&this.doc));
        });
    }

    /// A URL is never spliced in as a block. It links whatever is selected, or
    /// lands as a link where the caret is — and only when the block it landed
    /// in held nothing else does it also offer to become a card, which is the
    /// one place a card would not eat a sentence.
    fn paste_url(&mut self, url: String, cx: &mut Context<Self>) {
        // The one paste people expect to *not* overwrite what they chose.
        if !self.selection.is_collapsed() {
            return self.toggle_mark(Mark::Link(url), cx);
        }
        // A card needs a block with nothing else in it; a chip needs a body or
        // a cell to sit in. A fence holds its URL literally and offers neither.
        let at = self.cursor();
        let alone = at.part == Part::Body && self.caret_text().is_some_and(Text::is_empty);
        self.edit(EditKind::Structure, cx, |this| {
            let head = this.doc.replace(this.selection, Text::link(&url));
            this.selection = Selection::at(head.clamp(&this.doc));
        });
        if at.part != Part::Code {
            self.pasted = Some(link::Paste::open(at, url, alone));
            cx.notify();
        }
    }

    /// Answer the paste menu: leave the link, or turn its block into a card.
    pub(super) fn confirm_paste(&mut self, choice: Choice, cx: &mut Context<Self>) {
        let Some(pasted) = self.pasted.take() else {
            return;
        };
        let ix = pasted.at.block;
        match choice {
            Choice::Dismiss => cx.notify(),
            // A chip with a line to itself is a block, which is what gives it
            // room for a favicon; inside a sentence it is a mark over the text
            // that is already there, and only the spelling changes.
            Choice::Chip if pasted.alone => self.turn_into(ix, pasted.url, Form::Chip, cx),
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
                let head = this.doc.replace(Selection::new(pasted.at, end), text);
                this.selection = Selection::at(head.clamp(&this.doc));
            }),
            Choice::Bookmark => self.turn_into(ix, pasted.url, Form::Auto, cx),
            Choice::Embed => self.turn_into(ix, pasted.url, Form::Embed, cx),
        }
    }

    /// Give a block over to the link it holds.
    ///
    /// One step, not two: turning the block and giving the caret somewhere to
    /// go are one gesture, and undo has to agree.
    fn turn_into(&mut self, ix: usize, url: String, form: Form, cx: &mut Context<Self>) {
        self.edit(EditKind::Structure, cx, |this| {
            this.doc.set_kind(ix, BlockKind::Bookmark { url, form });
            // A block like this holds no caret, so the caret carries on in the
            // one after it — a fresh one when it ends the document.
            if this.doc.blocks.len() <= ix + 1 {
                this.doc
                    .blocks
                    .push(markdown::Block::new(BlockKind::Paragraph(Text::default())));
            }
            this.selection = Selection::at(Cursor::new(ix + 1, Part::Body, 0).clamp(&this.doc));
        });
    }

    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        if let Some((doc, selection)) = self.history.undo(&self.doc, self.selection) {
            self.doc = doc;
            self.selection = selection.clamp(&self.doc);
            cx.notify();
        }
    }

    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        if let Some((doc, selection)) = self.history.redo(&self.doc, self.selection) {
            self.doc = doc;
            self.selection = selection.clamp(&self.doc);
            cx.notify();
        }
    }

    /// Move the caret's block, children and all, and follow it.
    ///
    /// Public because a gutter handle and a menu row reach the same operation
    /// as the key does — one vocabulary, not three paths into [`Doc`].
    pub fn move_block(&mut self, ix: usize, delta: isize, cx: &mut Context<Self>) {
        self.edit(EditKind::Structure, cx, |this| {
            let caret = this.cursor();
            if let Some(to) = this.doc.move_block(ix, delta) {
                // The caret rides along, keeping its depth within the subtree
                // that moved and its offset within its own text.
                let block = to + caret.block.saturating_sub(ix);
                this.selection = Selection::at(Cursor { block, ..caret }.clamp(&this.doc));
            }
        });
    }

    pub fn duplicate_block(&mut self, ix: usize, cx: &mut Context<Self>) {
        self.edit(EditKind::Structure, cx, |this| {
            if let Some(copy) = this.doc.duplicate(ix) {
                this.selection = Selection::at(Cursor::new(copy, Part::Body, 0).clamp(&this.doc));
            }
        });
    }

    pub fn remove_block(&mut self, ix: usize, cx: &mut Context<Self>) {
        self.edit(EditKind::Structure, cx, |this| {
            this.doc.remove_block(ix);
            this.selection =
                Selection::at(Cursor::new(ix.saturating_sub(1), Part::Body, 0).clamp(&this.doc));
        });
    }

    /// Tag a fenced block with the language it holds, or `None` for plain.
    pub fn set_language(&mut self, ix: usize, language: Option<String>, cx: &mut Context<Self>) {
        self.edit(EditKind::Structure, cx, |this| {
            this.doc.set_language(ix, language);
        });
    }

    /// Turn the caret's block into `kind` — what the slash menu and the block
    /// menu both do.
    pub fn set_block(&mut self, ix: usize, kind: BlockKind, cx: &mut Context<Self>) {
        self.edit(EditKind::Structure, cx, |this| {
            this.doc.set_kind(ix, kind);
            this.selection = this.selection.clamp(&this.doc);
        });
    }
    /// The paragraph a document ending in a fence, a table, a rule or an image
    /// has no other way to grow: a fence swallows Enter, a cell has nowhere to
    /// put one, and a rule or an image holds no caret at all. `false` when the
    /// last block ends in a body, which can carry on by itself.
    fn append_tail(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(last) = self.doc.blocks.len().checked_sub(1) else {
            return false;
        };
        if self.doc.blocks[last].parts().last() == Some(&Part::Body) {
            return false;
        }
        self.edit(EditKind::Structure, cx, |this| {
            this.doc
                .blocks
                .push(markdown::Block::new(BlockKind::Paragraph(Text::default())));
            let ix = this.doc.blocks.len() - 1;
            this.selection = Selection::at(Cursor::new(ix, Part::Body, 0).clamp(&this.doc));
        });
        true
    }

    /// A click past the end of the document. Without this the document has no
    /// end: the click snaps back into the block above it, and what gets typed
    /// lands inside the code the reader was trying to escape.
    fn tail_click(&mut self, at: gpui::Point<gpui::Pixels>, cx: &mut Context<Self>) -> bool {
        let Some(last) = self.doc.blocks.len().checked_sub(1) else {
            return false;
        };
        let Some(bounds) = self.layouts.block_bounds(last) else {
            return false;
        };
        at.y > bounds.origin.y + bounds.size.height && self.append_tail(cx)
    }

    /// The caret's text, for the input handler's offset arithmetic.
    fn caret_text(&self) -> Option<&Text> {
        let at = self.cursor();
        self.doc.blocks.get(at.block)?.text_at(at.part)
    }
}

impl Focusable for Editor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let focused = self.focus_handle.is_focused(window);
        let selection = focused.then_some(self.selection);

        // Typed text and IME reach an entity only through an input handler
        // registered during *paint*, against the bounds it should be anchored
        // to. There is no custom element here to do that from, so a zero-cost
        // canvas over the document supplies the paint phase. Without this the
        // key bindings still fire and nothing types.
        let handle = self.focus_handle.clone();
        let entity = cx.entity();
        let input = canvas(
            |_, _, _| (),
            move |bounds, _, window, cx| {
                // The gutter handle is placed from positions recorded in window
                // coordinates, so the box they have to be measured against is
                // taken here — the one place that knows it.
                entity.update(cx, |this, _| this.origin = bounds.origin);
                window.handle_input(
                    &handle,
                    ElementInputHandler::new(bounds, entity.clone()),
                    cx,
                );
            },
        )
        .absolute()
        .size_full();

        // A tab stop, so the editor is reachable the same way every other
        // control in the library is.
        let handle = self.focus_handle.clone().tab_stop(true);

        div()
            .key_context(CONTEXT)
            .track_focus(&handle)
            // Tracking focus does not take it. Without this, clicking into the
            // document blurs the editor instead of putting a caret in it, and
            // the caret vanishes on the first click.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    // The handle's own listener runs first and claims the
                    // press; without the flag this would close the menu it
                    // just opened. `ui::popover::Popup` solves it the same way.
                    if std::mem::take(&mut this.handle_pressed) {
                        return;
                    }
                    this.block_menu = None;
                    this.language_menu = None;
                    this.pasted = None;
                    this.focus_handle.clone().focus(window, cx);
                    if this.tail_click(event.position, cx) {
                        return;
                    }
                    let Some(hit) = this.layouts.hit(event.position) else {
                        return cx.notify();
                    };
                    this.selection = match event.click_count {
                        // Shift extends from wherever the anchor already is,
                        // which is what makes click-then-shift-click a range.
                        _ if event.modifiers.shift => this.selection.extend_to(hit),
                        1 => Selection::at(hit),
                        2 => Selection::new(hit.word_left(&this.doc), hit.word_right(&this.doc)),
                        _ => Selection::new(hit.home(), hit.end(&this.doc)),
                    }
                    .clamp(&this.doc);
                    this.dragging = event.click_count == 1 && !event.modifiers.shift;
                    this.history.interrupt();
                    cx.notify();
                }),
            )
            // The drag has to be tracked from the container rather than from a
            // payload: a text selection has nothing to carry, and gpui's drag
            // payload is for things being dropped somewhere.
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                // Ahead of every drag branch below, because the pointer's shape
                // is about where it *is* rather than about what it is doing.
                let over_text = this.layouts.over_text(event.position);
                if over_text != this.over_text {
                    this.over_text = over_text;
                    cx.notify();
                }
                // A lifted block follows the pointer; otherwise the pointer
                // only decides which block wears the handle.
                if let Some((from, _)) = this.lifted.filter(|_| event.dragging()) {
                    if let Some(to) = this.layouts.block_at(event.position) {
                        this.lifted = Some((from, to));
                        cx.notify();
                    }
                    return;
                }
                if this.dragging && event.dragging() {
                    if let Some(hit) = this.layouts.hit(event.position) {
                        this.selection = this.selection.extend_to(hit).clamp(&this.doc);
                        cx.notify();
                    }
                    return;
                }
                let hovered = this.layouts.block_at(event.position);
                if hovered != this.hovered {
                    this.hovered = hovered;
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseUpEvent, _, cx| {
                    this.dragging = false;
                    let Some((from, to)) = this.lifted.take() else {
                        return;
                    };
                    if from == to {
                        // A press that never moved is a click, and a click on
                        // the handle is what opens the menu.
                        this.block_menu = Some((from, event.position));
                        return cx.notify();
                    }
                    this.edit(EditKind::Structure, cx, |this| {
                        // `move_block` steps one sibling at a time, so a drop
                        // several blocks away is that many steps. Bounded by
                        // the block count, which no drag can exceed.
                        let delta = if to > from { 1 } else { -1 };
                        let mut at = from;
                        for _ in 0..this.doc.blocks.len() {
                            let Some(next) = this.doc.move_block(at, delta) else {
                                break;
                            };
                            at = next;
                            if (delta > 0 && at >= to) || (delta < 0 && at <= to) {
                                break;
                            }
                        }
                        this.selection =
                            Selection::at(Cursor::new(at, Part::Body, 0).clamp(&this.doc));
                    });
                }),
            )
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(
                cx.listener(|this, _: &KillLine, _, cx| this.delete_to(true, Cursor::end, cx)),
            )
            .on_action(cx.listener(|this, _: &DeleteWordLeft, _, cx| {
                this.delete_to(false, Cursor::word_left, cx)
            }))
            .on_action(cx.listener(|this, _: &DeleteWordRight, _, cx| {
                this.delete_to(true, Cursor::word_right, cx)
            }))
            .on_action(cx.listener(|this, _: &DeleteToHome, _, cx| {
                this.delete_to(false, |at, _| at.home(), cx)
            }))
            .on_action(cx.listener(Self::split_block))
            .on_action(cx.listener(Self::indent))
            .on_action(cx.listener(Self::outdent))
            .on_action(cx.listener(Self::dismiss))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(|this, _: &ToggleBold, _, cx| this.toggle_mark(Mark::Bold, cx)))
            .on_action(
                cx.listener(|this, _: &ToggleItalic, _, cx| this.toggle_mark(Mark::Italic, cx)),
            )
            .on_action(
                cx.listener(|this, _: &ToggleStrike, _, cx| this.toggle_mark(Mark::Strike, cx)),
            )
            .on_action(cx.listener(|this, _: &ToggleCode, _, cx| this.toggle_mark(Mark::Code, cx)))
            .on_action(cx.listener(|this, _: &MoveBlockUp, _, cx| {
                this.move_block(this.cursor().block, -1, cx)
            }))
            .on_action(cx.listener(|this, _: &MoveBlockDown, _, cx| {
                this.move_block(this.cursor().block, 1, cx)
            }))
            .on_action(cx.listener(|this, _: &DuplicateBlock, _, cx| {
                this.duplicate_block(this.cursor().block, cx)
            }))
            .on_action(cx.listener(|this, _: &RemoveBlock, _, cx| {
                this.remove_block(this.cursor().block, cx)
            }))
            // Motion is one method with a `Cursor` function and an "extend"
            // flag, so a shift variant cannot drift from the key it shadows.
            .on_action(cx.listener(|this, _: &Left, _, cx| this.moved(false, Cursor::left, cx)))
            .on_action(cx.listener(|this, _: &Right, _, cx| this.moved(false, Cursor::right, cx)))
            .on_action(cx.listener(|this, _: &Up, _, cx| this.vertical(false, false, cx)))
            .on_action(cx.listener(|this, _: &Down, _, cx| this.vertical(true, false, cx)))
            .on_action(
                cx.listener(|this, _: &Home, _, cx| this.moved(false, |at, _| at.home(), cx)),
            )
            .on_action(cx.listener(|this, _: &End, _, cx| this.moved(false, Cursor::end, cx)))
            .on_action(
                cx.listener(|this, _: &WordLeft, _, cx| this.moved(false, Cursor::word_left, cx)),
            )
            .on_action(
                cx.listener(|this, _: &WordRight, _, cx| this.moved(false, Cursor::word_right, cx)),
            )
            .on_action(
                cx.listener(|this, _: &SelectLeft, _, cx| this.moved(true, Cursor::left, cx)),
            )
            .on_action(
                cx.listener(|this, _: &SelectRight, _, cx| this.moved(true, Cursor::right, cx)),
            )
            .on_action(cx.listener(|this, _: &SelectUp, _, cx| this.vertical(false, true, cx)))
            .on_action(cx.listener(|this, _: &SelectDown, _, cx| this.vertical(true, true, cx)))
            .on_action(
                cx.listener(|this, _: &SelectHome, _, cx| this.moved(true, |at, _| at.home(), cx)),
            )
            .on_action(cx.listener(|this, _: &SelectEnd, _, cx| this.moved(true, Cursor::end, cx)))
            .on_action(cx.listener(|this, _: &SelectWordLeft, _, cx| {
                this.moved(true, Cursor::word_left, cx)
            }))
            .on_action(cx.listener(|this, _: &SelectWordRight, _, cx| {
                this.moved(true, Cursor::word_right, cx)
            }))
            .w_full()
            // Text under the pointer, so the pointer says so — and only there,
            // or while a drag is still sweeping one out. The editor's box
            // reaches over its gutter, the margin beside a short line, a rule,
            // an image and a card, none of which a caret can be put into.
            // Where it has nothing to say it stays quiet rather than
            // overriding the page with an arrow of its own.
            .when(self.over_text || self.dragging, |el| {
                el.cursor(CursorStyle::IBeam)
            })
            // No focus ring. A ring says *widget*, and a document is not one —
            // the caret already paints only while focused, so a box around the
            // whole page is a second, louder signal for the same fact.
            .relative()
            .child(input)
            // The document is inset by the gutter so the handle has somewhere
            // to sit *inside* the editor. Outside it the handle is clipped by
            // any scrolling ancestor, and a drag through it never reaches
            // `on_mouse_move`, which fires only while this element is the one
            // under the pointer.
            .child(div().w_full().pl(gpui::px(HANDLE_GUTTER)).child(
                markdown::render_with_selection(
                    &self.doc,
                    selection,
                    Some(&self.layouts),
                    focused.then(|| PLACEHOLDER.into()),
                    window,
                    cx,
                ),
            ))
            // Last, so the layouts it reads are this frame's rather than the
            // one before — children paint in order.
            .child(
                canvas(|_, _, _| (), {
                    let entity = cx.entity();
                    move |_, _, _, cx| {
                        entity.update(cx, |this, cx| this.reveal_caret(cx));
                    }
                })
                .absolute()
                .size(gpui::px(0.0)),
            )
            .children(self.slash_menu(&theme, cx))
            .children(self.paste_menu(&theme, cx))
            .children(self.handle(&theme, cx))
            .children(self.drop_indicator(&theme))
            .children(self.language_chip(&theme, cx))
            .children(self.block_menu(&theme, cx))
            .children(self.language_menu(&theme, cx))
    }
}
