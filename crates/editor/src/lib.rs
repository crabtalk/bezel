//! A Notion-style block editor.
//!
//! The document is the single source of truth. There is no `TextField` per
//! block: a caret is a `(block, offset)` pair into one [`Doc`], which is what
//! makes Enter split, Backspace merge and Tab indent into list operations
//! rather than negotiations between separate widgets each owning a string.
//!
//! Everything about *what* an edit does lives in `bezel-markdown`'s `edit`
//! module, and is tested there without a window. This crate owns only what
//! needs one: a focus handle, key bindings, the platform input handler, and
//! turning a click into an offset.
//!
//! ```ignore
//! bezel_editor::init(cx);                       // once, at startup
//! let editor = cx.new(|cx| Editor::new("# Title", cx));
//! ```

use bezel_markdown::edit::{Cursor, shortcut};
use bezel_markdown::{BlockKind, BlockLayouts, Doc, Text};
use bezel_theme::Theme;
use gpui::{
    App, Context, CursorStyle, ElementInputHandler, Entity, EntityInputHandler, FocusHandle,
    Focusable, KeyBinding, MouseButton, Render, SharedString, Styled as _, UTF16Selection, Window,
    actions, canvas, div, prelude::*, px,
};
use std::ops::Range;

actions!(
    bezel_editor,
    [
        Backspace, Left, Right, Up, Down, Home, End, SplitBlock, Indent, Outdent,
    ]
);

/// Install the editor's key bindings. Scoped to the editor's own key context,
/// so binding `tab` here does not make `tab` mean "indent" for the whole app.
pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some(CONTEXT)),
        KeyBinding::new("left", Left, Some(CONTEXT)),
        KeyBinding::new("right", Right, Some(CONTEXT)),
        KeyBinding::new("up", Up, Some(CONTEXT)),
        KeyBinding::new("down", Down, Some(CONTEXT)),
        KeyBinding::new("home", Home, Some(CONTEXT)),
        KeyBinding::new("end", End, Some(CONTEXT)),
        KeyBinding::new("enter", SplitBlock, Some(CONTEXT)),
        KeyBinding::new("tab", Indent, Some(CONTEXT)),
        KeyBinding::new("shift-tab", Outdent, Some(CONTEXT)),
    ]);
}

const CONTEXT: &str = "BezelEditor";

pub struct Editor {
    doc: Doc,
    cursor: Cursor,
    focus_handle: FocusHandle,
    /// The IME composition range within the caret's block, underlined while it
    /// is being composed.
    marked: Option<Range<usize>>,
    /// Where each block's text landed last frame, so a click can be turned
    /// into a caret. Only paint knows this, so the renderer fills it.
    layouts: BlockLayouts,
}

impl Editor {
    pub fn new(source: &str, cx: &mut Context<Self>) -> Self {
        Self {
            doc: bezel_markdown::parse(source),
            cursor: Cursor::default(),
            focus_handle: cx.focus_handle(),
            marked: None,
            layouts: BlockLayouts::default(),
        }
    }

    pub fn doc(&self) -> &Doc {
        &self.doc
    }

    /// The document as markdown — normalized, because that is the form that
    /// survives being read back.
    pub fn source(&self) -> String {
        let mut doc = self.doc.clone();
        doc.normalize();
        bezel_markdown::serialize(&doc)
    }

    /// Insert text at the caret, applying a markdown prefix if one completes.
    fn insert(&mut self, text: &str, cx: &mut Context<Self>) {
        let at = self.cursor;
        self.doc
            .edit_text(at.block, |block| block.insert(at.offset, text));
        self.cursor.offset = at.offset + text.len();
        self.apply_shortcut();
        cx.notify();
    }

    /// Turn a typed prefix into the block it spells — `## ` into a heading.
    ///
    /// Runs after every insertion rather than only on space, because the
    /// vocabulary includes prefixes that end in one (`- [ ] `) and prefixes
    /// that do not (```` ``` ````).
    fn apply_shortcut(&mut self) {
        let Some(block) = self.doc.blocks.get(self.cursor.block) else {
            return;
        };
        let Some(text) = block.text() else { return };
        // Only from the very start of a block, and only up to the caret: a
        // `- ` typed in the middle of a sentence is a hyphen.
        let Some((hit, len)) = shortcut(&text.text) else {
            return;
        };
        if self.cursor.offset < len {
            return;
        }
        let mut rest = text.clone();
        rest.remove(0..len);
        let indent = block.indent;
        self.doc.blocks[self.cursor.block].kind = hit.apply(rest);
        self.doc.blocks[self.cursor.block].indent = indent;
        self.cursor.offset -= len;
        self.doc.repair();
    }

    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        let at = self.cursor;
        if at.offset > 0 {
            let to = at.left(&self.doc);
            self.doc
                .edit_text(at.block, |text| text.remove(to.offset..at.offset));
            self.cursor = to;
        } else if let Some(offset) = self.doc.merge_back(at.block) {
            // `merge_back` outdents, unmarkers, or merges — whichever the
            // block's state calls for — and says where the caret landed.
            self.cursor = Cursor::new(
                at.block.min(self.doc.blocks.len().saturating_sub(1)),
                offset,
            );
            if self.doc.blocks.len() < at.block + 1 {
                self.cursor.block = at.block.saturating_sub(1);
            }
        }
        self.cursor = self.cursor.clamp(&self.doc);
        cx.notify();
    }

    fn split_block(&mut self, _: &SplitBlock, _: &mut Window, cx: &mut Context<Self>) {
        let at = self.cursor;
        let new = self.doc.split(at.block, at.offset);
        self.cursor = Cursor::new(new, 0);
        cx.notify();
    }

    fn indent(&mut self, _: &Indent, _: &mut Window, cx: &mut Context<Self>) {
        self.doc.indent(self.cursor.block);
        cx.notify();
    }

    fn outdent(&mut self, _: &Outdent, _: &mut Window, cx: &mut Context<Self>) {
        self.doc.outdent(self.cursor.block);
        cx.notify();
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        self.cursor = self.cursor.left(&self.doc);
        cx.notify();
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        self.cursor = self.cursor.right(&self.doc);
        cx.notify();
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.cursor = self.cursor.up(&self.doc);
        cx.notify();
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.cursor = self.cursor.down(&self.doc);
        cx.notify();
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.cursor = self.cursor.home();
        cx.notify();
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.cursor = self.cursor.end(&self.doc);
        cx.notify();
    }

    /// The caret's block text, for the input handler's offset arithmetic.
    fn caret_text(&self) -> Option<&Text> {
        self.doc.blocks.get(self.cursor.block)?.text()
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
        let caret = focused.then_some(self.cursor);

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
                    this.focus_handle.clone().focus(window, cx);
                    if let Some(hit) = this.layouts.hit(event.position) {
                        this.cursor = hit.clamp(&this.doc);
                    }
                    cx.notify();
                }),
            )
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::split_block))
            .on_action(cx.listener(Self::indent))
            .on_action(cx.listener(Self::outdent))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
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
            .child(bezel_markdown::render_with_caret(
                &self.doc,
                caret,
                Some(&self.layouts),
                window,
                cx,
            ))
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
        Some(UTF16Selection {
            range: self.cursor.offset..self.cursor.offset,
            reversed: false,
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
        let replace = range.or_else(|| self.marked.clone());
        if let Some(range) = replace {
            let at = self.cursor.block;
            self.doc.edit_text(at, |block| block.remove(range.clone()));
            self.cursor.offset = range.start;
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
        let replace = range.or_else(|| self.marked.clone());
        if let Some(range) = replace {
            let at = self.cursor.block;
            self.doc.edit_text(at, |block| block.remove(range.clone()));
            self.cursor.offset = range.start;
        }
        let start = self.cursor.offset;
        let at = self.cursor.block;
        self.doc.edit_text(at, |block| block.insert(start, text));
        self.marked = Some(start..start + text.len());
        self.cursor.offset = marked
            .map(|range| start + range.end.min(text.len()))
            .unwrap_or(start + text.len());
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: gpui::Bounds<gpui::Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<gpui::Bounds<gpui::Pixels>> {
        None
    }

    fn character_index_for_point(
        &mut self,
        _: gpui::Point<gpui::Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        None
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
