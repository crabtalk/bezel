//! A Notion-style block editor. Behind the `editor` feature, off by default —
//! a reader that only paints a document never compiles it.
//!
//! The document is the single source of truth. There is no `TextField` per
//! block: a selection is a pair of [`Cursor`]s into one [`Doc`], which is what
//! makes Enter split, Backspace merge and Tab indent into list operations
//! rather than negotiations between separate widgets each owning a string.
//!
//! Everything about *what* an edit does lives in [`crate::edit`] and
//! [`crate::select`], and is tested there without a window. This module owns
//! only what needs one: a focus handle, key bindings, the platform input
//! handler, and turning a click into a position.
//!
//! ```ignore
//! markdown::editor::init(cx);             // once, at startup
//! let editor = cx.new(|cx| Editor::new("# Title", cx));
//! ```

use theme::Theme;

use crate::{
    BlockKind, BlockLayouts, Doc, Mark, Part, Text,
    edit::{self, shortcut},
    history::{EditKind, History},
    select::{Cursor, Selection},
    slash::Slash,
};
use gpui::{
    App, Context, CursorStyle, ElementInputHandler, Entity, EntityInputHandler, FocusHandle,
    Focusable, KeyBinding, MouseButton, Render, SharedString, Styled as _, UTF16Selection, Window,
    actions, canvas, div, prelude::*, px,
};
use std::ops::Range;

actions!(
    bezel_editor,
    [
        Backspace,
        Delete,
        Left,
        Right,
        Up,
        Down,
        Home,
        End,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectHome,
        SelectEnd,
        SelectAll,
        WordLeft,
        WordRight,
        SelectWordLeft,
        SelectWordRight,
        SplitBlock,
        Indent,
        Outdent,
        Dismiss,
        Copy,
        Cut,
        Paste,
        Undo,
        Redo,
        ToggleBold,
        ToggleItalic,
        ToggleStrike,
        ToggleCode,
        MoveBlockUp,
        MoveBlockDown,
        DuplicateBlock,
        RemoveBlock,
    ]
);

/// Install the editor's key bindings. Scoped to the editor's own key context,
/// so binding `tab` here does not make `tab` mean "indent" for the whole app.
///
/// The chords are [`ui::TextField`]'s, because a document is not the place to
/// invent a second set: what `alt-left` does in a search box is what a reader
/// expects it to do here.
pub fn init(cx: &mut App) {
    let ctx = Some(CONTEXT);
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, ctx),
        KeyBinding::new("delete", Delete, ctx),
        KeyBinding::new("left", Left, ctx),
        KeyBinding::new("right", Right, ctx),
        KeyBinding::new("up", Up, ctx),
        KeyBinding::new("down", Down, ctx),
        KeyBinding::new("home", Home, ctx),
        KeyBinding::new("end", End, ctx),
        KeyBinding::new("shift-left", SelectLeft, ctx),
        KeyBinding::new("shift-right", SelectRight, ctx),
        KeyBinding::new("shift-up", SelectUp, ctx),
        KeyBinding::new("shift-down", SelectDown, ctx),
        KeyBinding::new("shift-home", SelectHome, ctx),
        KeyBinding::new("shift-end", SelectEnd, ctx),
        KeyBinding::new("enter", SplitBlock, ctx),
        KeyBinding::new("tab", Indent, ctx),
        KeyBinding::new("shift-tab", Outdent, ctx),
        KeyBinding::new("escape", Dismiss, ctx),
    ]);

    // `MoveBlockUp`, `MoveBlockDown`, `DuplicateBlock` and `RemoveBlock` are
    // deliberately unbound. Every chord that fits is already taken by something
    // standard — `cmd-shift-up`/`down` select to the ends of a document on
    // macOS, `alt-up`/`down` move by paragraph — and shadowing one of those in
    // a text surface is worse than reaching the block menu for it. They are
    // actions so an app can bind what suits its own keymap.

    #[cfg(target_os = "macos")]
    cx.bind_keys([
        KeyBinding::new("cmd-a", SelectAll, ctx),
        KeyBinding::new("cmd-c", Copy, ctx),
        KeyBinding::new("cmd-x", Cut, ctx),
        KeyBinding::new("cmd-v", Paste, ctx),
        KeyBinding::new("cmd-z", Undo, ctx),
        KeyBinding::new("cmd-shift-z", Redo, ctx),
        KeyBinding::new("cmd-b", ToggleBold, ctx),
        KeyBinding::new("cmd-i", ToggleItalic, ctx),
        KeyBinding::new("cmd-e", ToggleCode, ctx),
        KeyBinding::new("cmd-shift-x", ToggleStrike, ctx),
        // cmd = line, option = word: the macOS convention.
        KeyBinding::new("cmd-left", Home, ctx),
        KeyBinding::new("cmd-right", End, ctx),
        KeyBinding::new("cmd-shift-left", SelectHome, ctx),
        KeyBinding::new("cmd-shift-right", SelectEnd, ctx),
        KeyBinding::new("alt-left", WordLeft, ctx),
        KeyBinding::new("alt-right", WordRight, ctx),
        KeyBinding::new("alt-shift-left", SelectWordLeft, ctx),
        KeyBinding::new("alt-shift-right", SelectWordRight, ctx),
        // The emacs bindings macOS honours in every native text field.
        KeyBinding::new("ctrl-a", Home, ctx),
        KeyBinding::new("ctrl-e", End, ctx),
        KeyBinding::new("ctrl-b", Left, ctx),
        KeyBinding::new("ctrl-f", Right, ctx),
        KeyBinding::new("ctrl-n", Down, ctx),
        KeyBinding::new("ctrl-p", Up, ctx),
        KeyBinding::new("ctrl-h", Backspace, ctx),
        KeyBinding::new("ctrl-d", Delete, ctx),
    ]);

    #[cfg(not(target_os = "macos"))]
    cx.bind_keys([
        KeyBinding::new("ctrl-a", SelectAll, ctx),
        KeyBinding::new("ctrl-c", Copy, ctx),
        KeyBinding::new("ctrl-x", Cut, ctx),
        KeyBinding::new("ctrl-v", Paste, ctx),
        KeyBinding::new("ctrl-z", Undo, ctx),
        KeyBinding::new("ctrl-shift-z", Redo, ctx),
        KeyBinding::new("ctrl-b", ToggleBold, ctx),
        KeyBinding::new("ctrl-i", ToggleItalic, ctx),
        KeyBinding::new("ctrl-e", ToggleCode, ctx),
        KeyBinding::new("ctrl-shift-x", ToggleStrike, ctx),
        // ctrl = word on Windows/Linux, where there is no line modifier.
        KeyBinding::new("ctrl-left", WordLeft, ctx),
        KeyBinding::new("ctrl-right", WordRight, ctx),
        KeyBinding::new("ctrl-shift-left", SelectWordLeft, ctx),
        KeyBinding::new("ctrl-shift-right", SelectWordRight, ctx),
    ]);
}

const CONTEXT: &str = "BezelEditor";

/// Shown on the focused block while it is empty — the only discoverable place
/// to say that `/` does anything.
const PLACEHOLDER: &str = "Type / for commands";

/// The handle's box, and how far left of the text it sits. Wide enough to be
/// hit without crowding the margin.
const HANDLE_SIZE: f32 = 18.0;
const HANDLE_GUTTER: f32 = 22.0;

/// Deliberately narrow: a scheme and no whitespace. Anything cleverer starts
/// linking text that merely contains a dot.
fn is_url(source: &str) -> bool {
    let source = source.trim();
    !source.is_empty()
        && !source.contains(char::is_whitespace)
        && (source.starts_with("http://") || source.starts_with("https://"))
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
    /// The block the pointer is over, which is the only one showing a handle.
    hovered: Option<usize>,
    /// A block being dragged by its handle, and where it would land.
    lifted: Option<(usize, usize)>,
    /// The block menu the handle opened, and where to anchor it.
    block_menu: Option<(usize, gpui::Point<gpui::Pixels>)>,
    /// Set by the handle's press so the editor's own press does not undo it.
    handle_pressed: bool,
    /// Where the editor's own box starts, so a position recorded in window
    /// coordinates can be placed inside it.
    origin: gpui::Point<gpui::Pixels>,
    /// Whether the pointer is dragging out a selection.
    dragging: bool,
    /// The column vertical motion is trying to keep, in pixels. Held across a
    /// run of up/down so walking through a short line and out the other side
    /// returns to the column you started in, and dropped by anything
    /// horizontal — which is every other way the caret moves.
    goal_x: Option<gpui::Pixels>,
}

impl Editor {
    pub fn new(source: &str, cx: &mut Context<Self>) -> Self {
        Self {
            doc: crate::parse(source),
            selection: Selection::default(),
            focus_handle: cx.focus_handle(),
            marked: None,
            layouts: BlockLayouts::default(),
            history: History::default(),
            stored: Vec::new(),
            slash: None,
            hovered: None,
            lifted: None,
            block_menu: None,
            handle_pressed: false,
            origin: gpui::Point::default(),
            dragging: false,
            goal_x: None,
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

    pub fn doc(&self) -> &Doc {
        &self.doc
    }

    pub fn selection(&self) -> Selection {
        self.selection
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
        // Every horizontal motion drops the column; the two vertical ones put
        // it back after calling this.
        self.goal_x = None;
        cx.notify();
    }

    fn head_to(&mut self, head: Cursor, extend: bool) {
        self.selection = if extend {
            self.selection.extend_to(head)
        } else {
            Selection::at(head)
        };
        // A motion ends the undo group: typing a word, moving away and typing
        // again must not undo as one step across two places. It also spends any
        // stored mark, which belonged to the spot the caret just left.
        self.history.interrupt();
        self.stored.clear();
    }

    /// Every mutation goes through here, so none of them can forget to record
    /// a step and none of them has to know how steps coalesce.
    fn edit(&mut self, kind: EditKind, cx: &mut Context<Self>, edit: impl FnOnce(&mut Self)) {
        self.history.record(kind, &self.doc, self.selection);
        edit(self);
        self.history.landed(kind, self.selection);
        cx.notify();
    }

    /// Up and down, by one painted row.
    ///
    /// Geometry rather than arithmetic on line numbers, so a wrapped paragraph,
    /// a code block's lines and a table's rows are all the same case and none
    /// needs counting. Falls back to the block-wise motion when the target is
    /// off the painted area — the first frame, or above the first line.
    fn vertical(&mut self, rows: f32, extend: bool, cx: &mut Context<Self>) {
        // Up and down walk the menu while it is open, not the document.
        if let Some(slash) = &mut self.slash {
            slash.filter.step(rows as isize);
            return cx.notify();
        }
        let head = self.selection.head;
        let Some((at, line_height)) = self.layouts.position(head) else {
            return self.moved(
                extend,
                |at, doc| if rows < 0.0 { at.up(doc) } else { at.down(doc) },
                cx,
            );
        };
        let goal = self.goal_x.unwrap_or(at.x);
        let target = gpui::point(goal, at.y + line_height * rows);
        match self.layouts.hit(target) {
            Some(hit) if hit != head => self.head_to(hit.clamp(&self.doc), extend),
            // Off the top is the start of the document and off the bottom is
            // its end, which is what every native field does.
            _ => {
                let to = if rows < 0.0 {
                    head.up(&self.doc)
                } else {
                    head.down(&self.doc)
                };
                self.head_to(to.clamp(&self.doc), extend);
            }
        }
        self.goal_x = Some(goal);
        cx.notify();
    }

    /// The document as markdown — normalized, because that is the form that
    /// survives being read back.
    pub fn source(&self) -> String {
        let mut doc = self.doc.clone();
        doc.normalize();
        crate::serialize(&doc)
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
                typed.marks.push(crate::MarkSpan {
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
    fn confirm_slash(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(slash) = &self.slash else {
            return false;
        };
        let (at, kind) = (slash.at, slash.choice());
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

    fn toggle_mark(&mut self, mark: Mark, cx: &mut Context<Self>) {
        // With nothing selected there is no range to mark, so the mark waits
        // for the next character — ProseMirror's stored marks, and the only way
        // cmd-B before typing can mean anything.
        if self.selection.is_collapsed() {
            match self.stored.iter().position(|stored| *stored == mark) {
                Some(ix) => drop(self.stored.remove(ix)),
                None => self.stored.push(mark),
            }
            return cx.notify();
        }
        let selection = self.selection;
        self.edit(EditKind::Structure, cx, |this| {
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

    /// Delete backwards: the selection if there is one, otherwise the character
    /// before the caret, otherwise whatever the start of a block means.
    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
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
        // The menu owns Enter while it is open, or picking a block would also
        // split the one it is turning.
        if self.confirm_slash(cx) {
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

    /// Escape closes the menu, and otherwise collapses a selection — the two
    /// things there are to back out of.
    fn dismiss(&mut self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        if self.slash.take().is_none() {
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
            crate::serialize(&slice)
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
        // A URL dropped on a selection links it rather than replacing it —
        // the one paste people expect to *not* overwrite what they chose.
        if !self.selection.is_collapsed() && is_url(&source) {
            return self.toggle_mark(Mark::Link(source.trim().to_string()), cx);
        }
        self.edit(EditKind::Structure, cx, |this| {
            let head = this.doc.splice(this.selection, crate::parse(&source));
            this.selection = Selection::at(head.clamp(&this.doc));
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

    /// Turn the caret's block into `kind` — what the slash menu and the block
    /// menu both do.
    pub fn set_block(&mut self, ix: usize, kind: BlockKind, cx: &mut Context<Self>) {
        self.edit(EditKind::Structure, cx, |this| {
            this.doc.set_kind(ix, kind);
            this.selection = this.selection.clamp(&this.doc);
        });
    }

    /// The gutter handle, on the block under the pointer.
    ///
    /// One handle rather than one per block: only the hovered block shows it,
    /// so a single element placed from the recorded frames does the whole job
    /// and the renderer stays clear of editor concerns.
    fn handle(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let ix = self.lifted.map(|(from, _)| from).or(self.hovered)?;
        let bounds = self.layouts.block_bounds(ix)?;
        Some(
            div()
                .id("block-handle")
                .absolute()
                .left(bounds.origin.x - self.origin.x - px(HANDLE_GUTTER))
                .top(bounds.origin.y - self.origin.y)
                .w(px(HANDLE_SIZE))
                .h(px(HANDLE_SIZE))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(4.0))
                .cursor(CursorStyle::OpenHand)
                .text_size(px(12.0))
                .text_color(theme.text_faint)
                .hover(|el| el.bg(theme.ink(0.08)).text_color(theme.text_muted))
                .child("⠿")
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                        this.handle_pressed = true;
                        this.lifted = Some((ix, ix));
                        // A press that never moves is a click and leaves the
                        // menu open; the first drag move clears it.
                        this.block_menu = Some((ix, event.position));
                        cx.notify();
                    }),
                )
                .into_any_element(),
        )
    }

    /// The line showing where a lifted block would land.
    fn drop_indicator(&self, theme: &Theme) -> Option<gpui::AnyElement> {
        let (from, to) = self.lifted.filter(|(from, to)| from != to)?;
        let bounds = self.layouts.block_bounds(to)?;
        // Above the target when moving up, below it when moving down — which
        // is where the block actually ends up.
        let y = if to < from {
            bounds.origin.y
        } else {
            bounds.origin.y + bounds.size.height
        };
        Some(
            div()
                .absolute()
                .left(px(0.0))
                .top(y - self.origin.y - px(1.0))
                .w_full()
                .h(px(2.0))
                .rounded(px(1.0))
                .bg(theme.accent)
                .into_any_element(),
        )
    }

    /// Turn into / Duplicate / Delete, at the handle that opened it.
    fn block_menu(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let (ix, at) = self.block_menu?;
        let turns = crate::slash::items();
        let rows = turns.into_iter().map(|(label, kind)| {
            ui::popover::menu_row(theme, false, SharedString::from(format!("turn-{label}")))
                .id(SharedString::from(format!("turn-row-{label}")))
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.block_menu = None;
                    this.set_block(ix, kind.clone(), cx);
                }))
        });
        let action = |label: &'static str, run: fn(&mut Self, usize, &mut Context<Self>)| {
            ui::popover::menu_row(theme, false, SharedString::from(format!("block-{label}")))
                .id(SharedString::from(format!("block-row-{label}")))
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.block_menu = None;
                    run(this, ix, cx);
                }))
        };
        Some(ui::popover::menu_at(
            "block-menu",
            at,
            div()
                .w(px(190.0))
                .max_h(px(320.0))
                .overflow_hidden()
                .child(ui::popover::menu_heading(theme, "Turn into"))
                .children(rows)
                .child(ui::popover::menu_heading(theme, "Block"))
                .child(action("Duplicate", |this, ix, cx| {
                    this.duplicate_block(ix, cx)
                }))
                .child(action("Delete", |this, ix, cx| this.remove_block(ix, cx)))
                .into_any_element(),
            None,
        ))
    }

    /// The menu, anchored under the `/` that opened it.
    ///
    /// The anchor comes from the same layout the caret paints against, so it
    /// costs nothing beyond a lookup and it cannot drift from the text.
    fn slash_menu(&self, theme: &Theme) -> Option<gpui::AnyElement> {
        let slash = self.slash.as_ref()?;
        let (point, line_height) = self.layouts.position(slash.at)?;
        let items = crate::slash::items();
        let rows = slash
            .filter
            .filtered()
            .iter()
            .enumerate()
            .map(|(row, &ix)| {
                ui::popover::menu_row(
                    theme,
                    Some(row) == slash.filter.active(),
                    SharedString::from(format!("slash-{ix}")),
                )
                .child(items[ix].0.clone())
            });
        Some(ui::popover::menu_at(
            "slash-menu",
            gpui::point(point.x, point.y + line_height),
            div()
                .w(px(200.0))
                .max_h(px(280.0))
                .overflow_hidden()
                .children(rows)
                .into_any_element(),
            None,
        ))
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
                    this.focus_handle.clone().focus(window, cx);
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
                // A lifted block follows the pointer; otherwise the pointer
                // only decides which block wears the handle.
                if let Some((from, _)) = this.lifted.filter(|_| event.dragging()) {
                    if let Some(to) = this.layouts.block_at(event.position) {
                        this.lifted = Some((from, to));
                        // Past the first move it is a drag, not a click.
                        this.block_menu = None;
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
                cx.listener(|this, _: &gpui::MouseUpEvent, _, cx| {
                    this.dragging = false;
                    let Some((from, to)) = this.lifted.take().filter(|(a, b)| a != b) else {
                        return;
                    };
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
            .on_action(cx.listener(|this, _: &Up, _, cx| this.vertical(-1.0, false, cx)))
            .on_action(cx.listener(|this, _: &Down, _, cx| this.vertical(1.0, false, cx)))
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
            .on_action(cx.listener(|this, _: &SelectUp, _, cx| this.vertical(-1.0, true, cx)))
            .on_action(cx.listener(|this, _: &SelectDown, _, cx| this.vertical(1.0, true, cx)))
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
            // Text under the pointer, so the pointer says so.
            .cursor(CursorStyle::IBeam)
            .p(px(2.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(if focused {
                theme.caret.opacity(0.5)
            } else {
                gpui::transparent_black()
            })
            .relative()
            .child(input)
            .child(crate::render_with_selection(
                &self.doc,
                selection,
                Some(&self.layouts),
                focused.then(|| PLACEHOLDER.into()),
                window,
                cx,
            ))
            .children(self.slash_menu(&theme))
            .children(self.handle(&theme, cx))
            .children(self.drop_indicator(&theme))
            .children(self.block_menu(&theme, cx))
    }
}

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

/// A label for the block the caret is in, so a host can show what typing will
/// produce without reaching into the document.
pub fn block_label(doc: &Doc, cursor: Cursor) -> SharedString {
    let Some(block) = doc.blocks.get(cursor.block) else {
        return "empty".into();
    };
    match &block.kind {
        BlockKind::Paragraph(_) => "Text",
        BlockKind::Heading { level, .. } => match level {
            1 => "Heading 1",
            2 => "Heading 2",
            _ => "Heading 3",
        },
        BlockKind::Bullet(_) => "Bullet",
        BlockKind::Ordered { .. } => "Numbered",
        BlockKind::Task { .. } => "Task",
        BlockKind::Quote(_) => "Quote",
        BlockKind::Code { .. } => "Code",
        BlockKind::Image { .. } => "Image",
        BlockKind::Table { .. } => "Table",
        BlockKind::Rule => "Divider",
    }
    .into()
}

/// A handle a host can keep without naming the entity type.
pub type EditorHandle = Entity<Editor>;
