//! The floating chrome: the gutter handle, the drop indicator, and the two
//! menus.
//!
//! All four are placed from positions `markdown::BlockLayouts` recorded as it
//! painted, so none of them can drift from the text it points at.

use crate::AppExt as _;
use gpui::{
    AnyElement, App, Context, CursorStyle, MouseButton, Pixels, Point, SharedString, Window, div,
    prelude::*, px,
};
use markdown::{Affinity, AppExt as _, BlockKind, Part};
use theme::{TextStyle, Theme, Typeset};
use ui::{
    icons::Icon,
    menu::{Cursor, Hit, Item},
    surface::Surfaced as _,
};

use crate::editor::{Dropped, Editor, HANDLE_SIZE, Line, TableTarget};
use crate::{SlashAction, SlashAt};

/// What a row of a table line's menu does.
type TableAction = Box<dyn Fn(&mut Editor, &mut Context<Editor>)>;

/// Add controls occupy reserved lanes inside the table block.
const TABLE_STRIP: f32 = markdown::render::TABLE_CONTROL_SIZE;
/// A table row or column handle, across and along the edge it sits on.
const TABLE_HANDLE_THIN: f32 = TABLE_STRIP;
const TABLE_HANDLE_LONG: f32 = 20.0;
const TABLE_HANDLE_GLYPH: f32 = 12.0;
const BLOCK_HANDLE_GLYPH: f32 = 14.0;

/// How far the language chip reaches past the word it wraps.
const CHIP_PAD_X: f32 = 6.0;
const CHIP_PAD_Y: f32 = 3.0;

/// Selector the interaction tests look the painted menu up by.
pub const SLASH_MENU: &str = "slash-menu";
/// The gutter handle's, for the same reason.
pub const BLOCK_HANDLE: &str = "block-handle";
/// The menu that handle opens, so a test can ask whether a second press on it
/// left the menu shut rather than closed-and-reopened.
pub const BLOCK_MENU: &str = "block-menu";
const TABLE_MENU: &str = "table-menu";
const LANGUAGE_MENU: &str = "language-menu";
const TEXT_MENU: &str = "text-menu";
const IMAGE_MENU: &str = "image-menu";

/// One of the menus a block's chrome drops.
#[derive(Clone, Copy)]
pub(super) enum Dropdown {
    Block,
    Table,
    Language,
    Text,
    Image,
}

impl Editor {
    /// The block the handle belongs on: the one being dragged, else the one
    /// under the pointer, else the one the caret is in.
    pub(super) fn handle_block(&self, focused: bool) -> Option<usize> {
        if !self.chrome().handle {
            return None;
        }
        if !self.blocks() {
            return None;
        }
        self.lifted
            .map(|(from, _)| from)
            .or(self.hovered)
            .or_else(|| focused.then(|| self.cursor().block))
    }

    /// Where the handle sits for that block, in this editor's own space.
    ///
    /// Centred on the block's first row, not dropped at the top of its box. A
    /// block with nothing painted in it has no row to centre on and keeps the
    /// box's top.
    pub(super) fn handle_origin(&self, ix: usize, cx: &App) -> Option<Point<Pixels>> {
        let bounds = self.layouts.block_bounds(ix)?;
        let top = match self.layouts.first_row(ix) {
            Some((row, line)) => row + (line - px(HANDLE_SIZE)) / 2.0,
            None => bounds.origin.y,
        };
        Some(gpui::point(
            bounds.origin.x - self.origin.x - px(cx.editor_layout().text_inset),
            top - self.origin.y,
        ))
    }

    /// The gutter handle, on the block being dragged, else the one under the
    /// pointer, else the one the caret is in.
    ///
    /// One handle rather than one per block: a single element placed from the
    /// recorded frames does the whole job and the renderer stays clear of
    /// editor concerns. The caret comes last because a pointer in the document
    /// is the more immediate intent — and it comes at all because reaching a
    /// block by keyboard should not mean reaching for the mouse to act on it.
    pub(super) fn handle(
        &mut self,
        focused: bool,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let placed = self
            .handle_block(focused)
            .and_then(|ix| Some((ix, self.handle_origin(ix, cx)?)));
        // Kept even when there is none, so [`Self::settle_handle`] compares
        // like with like and does not ask for a frame over a handle that is
        // not there.
        self.handle_at = placed.map(|(_, at)| at);
        let (ix, at) = placed?;
        let glyph = crate::handles::installed(cx).block;
        let handle = div()
            .id(BLOCK_HANDLE)
            .group(BLOCK_HANDLE)
            .debug_selector(|| BLOCK_HANDLE.to_string())
            .absolute()
            .left(at.x)
            .top(at.y)
            .w(px(HANDLE_SIZE))
            .h(px(HANDLE_SIZE))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.0))
            .cursor(CursorStyle::OpenHand)
            .child(
                ui::icons::icon(glyph)
                    .size(px(BLOCK_HANDLE_GLYPH))
                    .text_color(theme.text_faint)
                    .group_hover(BLOCK_HANDLE, |el| el.text_color(theme.text_muted)),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _: &gpui::MouseDownEvent, _, cx| {
                    this.press_claimed = true;
                    this.lifted = Some((ix, ix));
                    // The menu belongs to the release. Opened here it would
                    // occlude the very moves a drag downwards is made of,
                    // and the drop target would never leave the block it
                    // started on — so the press only dismisses, and the note
                    // `trigger_press_matching` took is what tells the
                    // release apart from a fresh open.
                    ui::popover::close_popup(this, cx, |this| &mut this.block_menu);
                    cx.notify();
                }),
            );
        Some(
            ui::popover::trigger_press_matching(
                handle,
                |this| &mut this.block_menu,
                move |menu| menu.of.0 == ix,
                cx,
            )
            .surface(theme, theme.popover_surface)
            .into_any_element(),
        )
    }

    /// The line showing where a lifted block — or a file dragged in from
    /// outside — would land.
    pub(super) fn drop_indicator(&self, theme: &Theme) -> Option<AnyElement> {
        if !self.blocks() {
            return None;
        }
        let (from, to) = match self.lifted.filter(|(from, to)| from != to) {
            Some(lifted) => lifted,
            // A file always lands under the block it is over, so it is a drag
            // that only ever moves downwards.
            None => self.dropping.map(|to| (to, to + 1))?,
        };
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
                .top(y - self.origin.y)
                .w_full()
                .h(px(1.0))
                .bg(theme.drop_line)
                .into_any_element(),
        )
    }

    /// The `+` strips along a table's right and bottom edges, on the table
    /// under the pointer or else the one the caret is in: a column and a row
    /// at the end.
    pub(super) fn table_strips(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.blocks() {
            return None;
        }
        let ix = [self.hovered, Some(self.cursor().block)]
            .into_iter()
            .flatten()
            .find(|ix| {
                matches!(
                    self.doc.blocks.get(*ix).map(|block| &block.kind),
                    Some(BlockKind::Table { .. })
                )
            })?;
        let BlockKind::Table { header, rows, .. } = &self.doc.blocks[ix].kind else {
            return None;
        };
        let columns = rows.iter().map(Vec::len).chain([header.len()]).max()?;
        let end_row = rows.len() + 1;
        let bounds = self.layouts.block_bounds(ix)?;
        let origin = bounds.origin - self.origin;
        let strip =
            |id: &'static str, add: fn(&mut Self, usize, usize, &mut Context<Self>), at: usize| {
                div()
                    .id(id)
                    .debug_selector(|| id.to_string())
                    .absolute()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.0))
                    .cursor(CursorStyle::PointingHand)
                    .text_style(TextStyle::Callout)
                    .text_color(theme.text_faint)
                    .hover(|el| el.bg(theme.element_hover).text_color(theme.text_muted))
                    .child("+")
                    // Held here, or the editor's own move handler takes the
                    // hovered block away from under the pointer.
                    .on_mouse_move(|_, _, cx| cx.stop_propagation())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _: &gpui::MouseDownEvent, window, cx| {
                            this.press_claimed = true;
                            this.focus_handle.clone().focus(window, cx);
                            add(this, ix, at, cx);
                        }),
                    )
            };
        Some(
            div()
                .absolute()
                .left(origin.x)
                .top(origin.y)
                .w(bounds.size.width)
                .h(bounds.size.height)
                .child(
                    strip("table-add-column", Self::add_column, columns)
                        .left(bounds.size.width - px(TABLE_STRIP))
                        .top(px(TABLE_STRIP))
                        .w(px(TABLE_STRIP))
                        .h(bounds.size.height - px(2.0 * TABLE_STRIP)),
                )
                .child(
                    strip("table-add-row", Self::add_row, end_row)
                        .left(px(TABLE_STRIP))
                        .top(bounds.size.height - px(TABLE_STRIP))
                        .w(bounds.size.width - px(2.0 * TABLE_STRIP))
                        .h(px(TABLE_STRIP)),
                )
                .into_any_element(),
        )
    }

    /// A handle on the table's left edge at the row of the hovered cell, else
    /// the caret's, and one on its top edge at that cell's column. Each opens
    /// its line's menu.
    pub(super) fn table_handles(
        &self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.blocks() {
            return None;
        }
        let caret = self.cursor();
        let (ix, part) = self.hovered_cell.or_else(|| {
            matches!(caret.part, Part::Cell { .. }).then_some((caret.block, caret.part))
        })?;
        let Part::Cell { row, column } = part else {
            return None;
        };
        let table = self.layouts.block_bounds(ix)?;
        let cell = self.layouts.cell_bounds(ix, part)?;
        let glyphs = crate::handles::installed(cx);
        let handle = |id: &'static str, glyph: Icon, line: TableTarget, anchor: Point<Pixels>| {
            let trigger = div()
                .id(id)
                .group(id)
                .debug_selector(|| id.to_string())
                .absolute()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(4.0))
                .cursor(CursorStyle::PointingHand)
                .child(
                    ui::icons::icon(glyph)
                        .size(px(TABLE_HANDLE_GLYPH))
                        .text_color(theme.text_faint)
                        .group_hover(id, |el| el.text_color(theme.text_muted)),
                )
                .on_mouse_move(|_, _, cx| cx.stop_propagation())
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                        this.press_claimed = true;
                        this.table_dragged = false;
                        let from = match line {
                            TableTarget::Row(row) => Line::Row(row),
                            TableTarget::Column(column) => Line::Column(column),
                            TableTarget::Cell { .. } => unreachable!(),
                        };
                        this.table_drag = Some(super::table::TableDrag {
                            block: ix,
                            line: from,
                            start: event.position,
                            to: None,
                        });
                        cx.notify();
                    }),
                );
            ui::popover::trigger_press_matching(
                trigger,
                |this| &mut this.table_menu,
                move |menu| menu.of == (ix, line),
                cx,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                if this.table_dragged {
                    return;
                }
                if this.table_menu.take_press_was_open() {
                    ui::popover::close_popup(this, cx, |this| &mut this.table_menu);
                } else {
                    this.table_menu.open(Dropped::new((ix, line), anchor));
                    cx.notify();
                }
            }))
        };
        let row_mid = cell.origin.y + cell.size.height / 2.0;
        let column_mid = cell.origin.x + cell.size.width / 2.0;
        Some(
            div()
                .absolute()
                .top(px(0.0))
                .left(px(0.0))
                .size_full()
                .child(
                    handle(
                        "table-row-handle",
                        glyphs.row.clone(),
                        TableTarget::Row(row),
                        gpui::point(table.origin.x, row_mid),
                    )
                    .left(table.origin.x - self.origin.x)
                    .top(row_mid - self.origin.y - px(TABLE_HANDLE_LONG / 2.0))
                    .w(px(TABLE_HANDLE_THIN))
                    .h(px(TABLE_HANDLE_LONG))
                    .surface(theme, theme.popover_surface),
                )
                .child(
                    handle(
                        "table-column-handle",
                        glyphs.column.clone(),
                        TableTarget::Column(column),
                        gpui::point(column_mid, table.origin.y),
                    )
                    .left(column_mid - self.origin.x - px(TABLE_HANDLE_LONG / 2.0))
                    .top(table.origin.y - self.origin.y)
                    .w(px(TABLE_HANDLE_LONG))
                    .h(px(TABLE_HANDLE_THIN))
                    .surface(theme, theme.popover_surface),
                )
                .into_any_element(),
        )
    }

    /// Insert, delete and move, for the line or cell of table `ix` a handle
    /// or a right click opened.
    fn table_actions(&self, ix: usize, line: TableTarget) -> Vec<(&'static str, TableAction)> {
        let Some(BlockKind::Table { header, rows, .. }) =
            self.doc.blocks.get(ix).map(|block| &block.kind)
        else {
            return Vec::new();
        };
        let width = rows
            .iter()
            .map(Vec::len)
            .chain([header.len()])
            .max()
            .unwrap_or(0);
        let action = |label: &'static str, run: TableAction| (label, run);
        let mut items: Vec<_> = match line {
            TableTarget::Cell { row, column } => [
                (row > 0).then(|| {
                    action(
                        "Insert above",
                        Box::new(move |this, cx| this.add_row(ix, row, cx)),
                    )
                }),
                Some(action(
                    "Insert below",
                    Box::new(move |this, cx| this.add_row(ix, row + 1, cx)),
                )),
                Some(action(
                    "Insert left",
                    Box::new(move |this, cx| this.add_column(ix, column, cx)),
                )),
                Some(action(
                    "Insert right",
                    Box::new(move |this, cx| this.add_column(ix, column + 1, cx)),
                )),
                (row > 0).then(|| {
                    action(
                        "Delete row",
                        Box::new(move |this, cx| this.remove_row(ix, row, cx)),
                    )
                }),
                (width > 1).then(|| {
                    action(
                        "Delete column",
                        Box::new(move |this, cx| this.remove_column(ix, column, cx)),
                    )
                }),
            ]
            .into_iter()
            .flatten()
            .collect(),
            TableTarget::Row(row) => [
                (row > 0).then(|| {
                    action(
                        "Insert above",
                        Box::new(move |this, cx| this.add_row(ix, row, cx)),
                    )
                }),
                Some(action(
                    "Insert below",
                    Box::new(move |this, cx| this.add_row(ix, row + 1, cx)),
                )),
                (row > 0).then(|| {
                    action(
                        "Delete row",
                        Box::new(move |this, cx| this.remove_row(ix, row, cx)),
                    )
                }),
            ]
            .into_iter()
            .flatten()
            .collect(),
            TableTarget::Column(column) => [
                Some(action(
                    "Insert left",
                    Box::new(move |this, cx| this.add_column(ix, column, cx)),
                )),
                Some(action(
                    "Insert right",
                    Box::new(move |this, cx| this.add_column(ix, column + 1, cx)),
                )),
                (width > 1).then(|| {
                    action(
                        "Delete column",
                        Box::new(move |this, cx| this.remove_column(ix, column, cx)),
                    )
                }),
            ]
            .into_iter()
            .flatten()
            .collect(),
        };
        let row = match line {
            TableTarget::Row(row) | TableTarget::Cell { row, .. } => Some(row),
            _ => None,
        };
        let column = match line {
            TableTarget::Column(column) | TableTarget::Cell { column, .. } => Some(column),
            _ => None,
        };
        if let Some(row) = row {
            if row > 1 {
                items.push(action(
                    "Move up",
                    Box::new(move |this, cx| this.move_row(ix, row, row - 1, cx)),
                ));
            }
            if row > 0 && row < rows.len() {
                items.push(action(
                    "Move down",
                    Box::new(move |this, cx| this.move_row(ix, row, row + 1, cx)),
                ));
            }
        }
        if let Some(column) = column {
            if column > 0 {
                items.push(action(
                    "Move left",
                    Box::new(move |this, cx| this.move_column(ix, column, column - 1, cx)),
                ));
            }
            if column + 1 < width {
                items.push(action(
                    "Move right",
                    Box::new(move |this, cx| this.move_column(ix, column, column + 1, cx)),
                ));
            }
        }
        items
    }

    /// The click target over a fence's header, where its language sits.
    ///
    /// The header paints the name; this only takes the press. A renderer holds
    /// a `&Doc` and cannot change one, which is why the copy button can live
    /// down there and a language picker cannot.
    pub(super) fn language_chip(
        &self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        // The source is a fence too, and it has no language to pick.
        if !self.chrome().language || !self.blocks() {
            return None;
        }
        // Whichever fence the reader is at: the one under the pointer, else the
        // one the caret is in.
        let ix = [self.hovered, Some(self.cursor().block)]
            .into_iter()
            .flatten()
            .find(|ix| {
                matches!(
                    self.doc.blocks.get(*ix).map(|b| &b.kind),
                    Some(BlockKind::Code { .. })
                )
            })?;
        // The label's box, grown by the chip's padding — so the wash is around
        // the word and nothing else, whatever the word is.
        let bounds = self.layouts.language_bounds(ix)?;
        let anchor = gpui::point(
            bounds.origin.x - px(CHIP_PAD_X),
            bounds.origin.y + bounds.size.height + px(CHIP_PAD_Y),
        );
        let chip = div()
            .id("language-chip")
            .absolute()
            .left(bounds.origin.x - self.origin.x - px(CHIP_PAD_X))
            .top(bounds.origin.y - self.origin.y - px(CHIP_PAD_Y))
            .w(bounds.size.width + px(2.0 * CHIP_PAD_X))
            .h(bounds.size.height + px(2.0 * CHIP_PAD_Y))
            .rounded(px(4.0))
            .cursor(CursorStyle::PointingHand)
            .hover(|el| el.bg(theme.element_hover));
        Some(
            ui::popover::menu_trigger_matching(
                chip,
                |this| &mut this.language_menu,
                move |menu| menu.of == ix,
                move |_| Dropped::new(ix, anchor),
                cx,
            )
            .into_any_element(),
        )
    }

    /// Plain, then the languages the installed highlighter knows, each with
    /// the tag it writes on the fence.
    fn languages(&self, cx: &App) -> Vec<(SharedString, Option<String>)> {
        std::iter::once((markdown::render::PLAIN_LANGUAGE.into(), None))
            .chain(
                cx.highlight_languages()
                    .iter()
                    .map(|name| (name.clone(), Some(name.to_string()))),
            )
            .collect()
    }

    /// The rows of a dropped menu, while it is mounted.
    fn dropdown_items(&self, which: Dropdown, cx: &App) -> Option<Vec<Item>> {
        match which {
            Dropdown::Text => Some(self.text_menu.get()?.of.clone()),
            Dropdown::Image => Some(self.image_menu.get()?.of.items()),
            _ if !self.blocks() => None,
            Dropdown::Block => Some(self.block_menu.get()?.of.1.menu()),
            Dropdown::Table => {
                let (ix, line) = self.table_menu.get()?.of;
                Some(
                    self.table_actions(ix, line)
                        .into_iter()
                        .map(|(label, _)| Item::action(label))
                        .collect(),
                )
            }
            Dropdown::Language => {
                if !self.chrome().language {
                    return None;
                }
                let ix = self.language_menu.get()?.of;
                let Some(BlockKind::Code { language, .. }) =
                    self.doc.blocks.get(ix).map(|b| &b.kind)
                else {
                    return None;
                };
                Some(
                    self.languages(cx)
                        .into_iter()
                        .map(|(label, tag)| Item::action(label).checked(tag == *language))
                        .collect(),
                )
            }
        }
    }

    fn dropdown_cursor(&mut self, which: Dropdown) -> Option<&mut Cursor> {
        match which {
            Dropdown::Block => self.block_menu.open_mut().map(|menu| &mut menu.cursor),
            Dropdown::Table => self.table_menu.open_mut().map(|menu| &mut menu.cursor),
            Dropdown::Language => self.language_menu.open_mut().map(|menu| &mut menu.cursor),
            Dropdown::Text => self.text_menu.open_mut().map(|menu| &mut menu.cursor),
            Dropdown::Image => self.image_menu.open_mut().map(|menu| &mut menu.cursor),
        }
    }

    fn close_dropdown(&mut self, which: Dropdown, cx: &mut Context<Self>) {
        match which {
            Dropdown::Block => ui::popover::close_popup(self, cx, |this| &mut this.block_menu),
            Dropdown::Table => ui::popover::close_popup(self, cx, |this| &mut this.table_menu),
            Dropdown::Language => {
                ui::popover::close_popup(self, cx, |this| &mut this.language_menu)
            }
            Dropdown::Text => ui::popover::close_popup(self, cx, |this| &mut this.text_menu),
            Dropdown::Image => ui::popover::close_popup(self, cx, |this| &mut this.image_menu),
        }
    }

    /// Close the menu and run the row at `path`.
    fn choose_dropdown(
        &mut self,
        which: Dropdown,
        path: &[usize],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(&row) = path.first() else { return };
        match which {
            Dropdown::Block => {
                let Some(menu) = self.block_menu.as_open() else {
                    return;
                };
                let (ix, action) = (menu.of.0, menu.of.1.action_at(path));
                let Some(action) = action else { return };
                self.close_dropdown(which, cx);
                self.block_menu_action(ix, action, window, cx);
            }
            Dropdown::Table => {
                let Some(&Dropped { of: (ix, line), .. }) = self.table_menu.as_open() else {
                    return;
                };
                let run = self.table_actions(ix, line).into_iter().nth(row);
                self.close_dropdown(which, cx);
                if let Some((_, run)) = run {
                    run(self, cx);
                }
            }
            Dropdown::Language => {
                let Some(&Dropped { of: ix, .. }) = self.language_menu.as_open() else {
                    return;
                };
                let tag = self.languages(cx).into_iter().nth(row);
                self.close_dropdown(which, cx);
                if let Some((_, tag)) = tag {
                    self.set_language(ix, tag, cx);
                }
            }
            Dropdown::Image => {
                let Some(menu) = self.image_menu.as_open() else {
                    return;
                };
                menu.of.run(row, cx);
                self.close_dropdown(which, cx);
            }
            Dropdown::Text => {
                self.close_dropdown(which, cx);
                match ui::menu::Edit::at(path) {
                    Some(ui::menu::Edit::Cut) => self.cut(&super::Cut, window, cx),
                    Some(ui::menu::Edit::Copy) => self.copy(&super::Copy, window, cx),
                    Some(ui::menu::Edit::Paste) => self.paste(&super::Paste, window, cx),
                    Some(ui::menu::Edit::SelectAll) => {
                        self.select_all(&super::SelectAll, window, cx)
                    }
                    None => {}
                }
            }
        }
    }

    fn dropdown_hit(
        &mut self,
        which: Dropdown,
        hit: Hit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match hit {
            Hit::Point(path) => {
                let Some(items) = self.dropdown_items(which, cx) else {
                    return;
                };
                if let Some(cursor) = self.dropdown_cursor(which)
                    && cursor.point_at(&items, &path)
                {
                    cx.notify();
                }
            }
            Hit::Choose(path) => self.choose_dropdown(which, &path, window, cx),
            Hit::Dismiss => self.close_dropdown(which, cx),
        }
    }

    /// The dropped menu the keyboard works, while one is open.
    fn open_dropdown(&self) -> Option<Dropdown> {
        [
            (Dropdown::Block, self.block_menu.is_open()),
            (Dropdown::Table, self.table_menu.is_open()),
            (Dropdown::Language, self.language_menu.is_open()),
            (Dropdown::Text, self.text_menu.is_open()),
            (Dropdown::Image, self.image_menu.is_open()),
        ]
        .into_iter()
        .find_map(|(which, open)| open.then_some(which))
    }

    /// Up and down in an open dropped menu. `false` when none is open.
    pub(super) fn dropdown_step(&mut self, delta: isize, cx: &mut Context<Self>) -> bool {
        let Some(which) = self.open_dropdown() else {
            return false;
        };
        let Some(items) = self.dropdown_items(which, cx) else {
            return false;
        };
        if let Some(cursor) = self.dropdown_cursor(which) {
            cursor.step(&items, delta);
            cx.notify();
        }
        true
    }

    /// Left and right out of and into an open dropped menu's submenu.
    pub(super) fn dropdown_side(&mut self, right: bool, cx: &mut Context<Self>) -> bool {
        let Some(which) = self.open_dropdown() else {
            return false;
        };
        let Some(items) = self.dropdown_items(which, cx) else {
            return false;
        };
        let Some(cursor) = self.dropdown_cursor(which) else {
            return false;
        };
        let moved = match right {
            true => cursor.descend(&items),
            false => cursor.ascend(),
        };
        if moved {
            cx.notify();
        }
        moved
    }

    /// Enter on an open dropped menu's live row: a submenu opens, anything
    /// else runs. `false` when no row is live.
    pub(super) fn dropdown_enter(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(which) = self.open_dropdown() else {
            return false;
        };
        let Some(items) = self.dropdown_items(which, cx) else {
            return false;
        };
        let Some(cursor) = self.dropdown_cursor(which) else {
            return false;
        };
        let Some(path) = cursor.path() else {
            return false;
        };
        if cursor.descend(&items) {
            cx.notify();
        } else {
            self.choose_dropdown(which, &path, window, cx);
        }
        true
    }

    /// Escape on an open dropped menu. `false` when none is open.
    pub(super) fn dropdown_dismiss(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(which) = self.open_dropdown() else {
            return false;
        };
        self.close_dropdown(which, cx);
        true
    }

    /// A dropped menu, at the anchor it opened with.
    pub(super) fn dropdown(
        &self,
        which: Dropdown,
        theme: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let items = self.dropdown_items(which, cx)?;
        let (id, menu, closing) = match which {
            Dropdown::Block => {
                let menu = self.block_menu.get()?;
                (
                    BLOCK_MENU,
                    (menu.at, &menu.cursor),
                    self.block_menu.closing_since(),
                )
            }
            Dropdown::Table => {
                let menu = self.table_menu.get()?;
                (
                    TABLE_MENU,
                    (menu.at, &menu.cursor),
                    self.table_menu.closing_since(),
                )
            }
            Dropdown::Language => {
                let menu = self.language_menu.get()?;
                let closing = self.language_menu.closing_since();
                (LANGUAGE_MENU, (menu.at, &menu.cursor), closing)
            }
            Dropdown::Text => {
                let menu = self.text_menu.get()?;
                let closing = self.text_menu.closing_since();
                (TEXT_MENU, (menu.at, &menu.cursor), closing)
            }
            Dropdown::Image => {
                let menu = self.image_menu.get()?;
                let closing = self.image_menu.closing_since();
                (IMAGE_MENU, (menu.at, &menu.cursor), closing)
            }
        };
        let (at, cursor) = menu;
        let card = ui::menu::card(
            theme,
            id,
            &items,
            cursor,
            window,
            cx,
            move |this: &mut Self, hit, window, cx| this.dropdown_hit(which, hit, window, cx),
        )
        .debug_selector(|| id.to_string())
        .max_h(px(320.0));
        Some(ui::popover::menu_at(
            id,
            at,
            card.into_any_element(),
            closing,
        ))
    }

    fn block_menu_action(
        &mut self,
        ix: usize,
        action: SlashAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            SlashAction::Block(kind) => self.set_block(ix, kind, cx),
            SlashAction::Quote(alert) => self.toggle_quote(ix, alert, cx),
            SlashAction::Run(run) => {
                // After this update ends: the app edits this editor.
                let editor = cx.entity().downgrade();
                window.defer(cx, move |window, cx| {
                    run(SlashAt { editor, block: ix }, window, cx)
                });
            }
        }
    }

    /// What a pasted URL could be, under the block it landed in.
    ///
    /// Anchored at the block's start rather than at the caret: the caret is
    /// past the end of a URL, which is as far right as a line goes, and a menu
    /// hanging off there points at nothing.
    pub(super) fn paste_menu(
        &self,
        theme: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let pasted = self.pasted.as_ref()?;
        let (point, line_height) = self.layouts.position(pasted.at, Affinity::Downstream)?;
        let card = ui::menu::card(
            theme,
            "paste",
            &pasted.menu(),
            &pasted.cursor,
            window,
            cx,
            |this: &mut Self, hit, _, cx| match hit {
                Hit::Point(path) => {
                    if let Some(pasted) = this.pasted.as_mut()
                        && pasted.cursor.point_at(&pasted.menu(), &path)
                    {
                        cx.notify();
                    }
                }
                Hit::Choose(path) => {
                    let choice = this.pasted.as_ref().and_then(|p| p.choice(Some(&path)));
                    if let Some(choice) = choice {
                        this.confirm_paste(choice, cx);
                    }
                }
                // Clicking away is `Dismiss`, which is a real answer: the link
                // is already in the block and stays there.
                Hit::Dismiss => {
                    this.pasted = None;
                    cx.notify();
                }
            },
        );
        Some(ui::popover::menu_at(
            "paste-menu",
            gpui::point(point.x, point.y + line_height),
            card.into_any_element(),
            None,
        ))
    }

    /// The menu, anchored under the `/` that opened it.
    ///
    /// The anchor comes from the same layout the caret paints against, so it
    /// costs nothing beyond a lookup and it cannot drift from the text.
    pub(super) fn slash_menu(
        &self,
        theme: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let slash = self.slash.as_ref()?;
        let (point, line_height) = self.layouts.position(slash.at, Affinity::Downstream)?;
        let card = ui::menu::card(
            theme,
            "slash",
            &slash.menu(),
            &slash.cursor,
            window,
            cx,
            |this: &mut Self, hit, window, cx| match hit {
                Hit::Point(path) => {
                    if let Some(slash) = this.slash.as_mut()
                        && slash.cursor.point_at(&slash.menu(), &path)
                    {
                        cx.notify();
                    }
                }
                Hit::Choose(path) => {
                    this.confirm_slash(Some(path.to_vec()), window, cx);
                }
                Hit::Dismiss => {
                    this.slash = None;
                    cx.notify();
                }
            },
        )
        // Compiles to nothing outside a test build. It is here because the
        // menu's state opening and the menu *painting* are two different
        // things, and the bug that shipped was the second one failing while
        // the first looked fine.
        .debug_selector(|| SLASH_MENU.to_string())
        .max_h(px(280.0));
        Some(ui::popover::menu_at(
            "slash-menu",
            gpui::point(point.x, point.y + line_height),
            card.into_any_element(),
            None,
        ))
    }

    /// The `@` menu, under the `@` the way [`Self::slash_menu`] hangs under
    /// its `/`. Nothing while the source has no rows for the query.
    pub(super) fn mention_menu(
        &self,
        theme: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let mention = self.mention.as_ref()?;
        let items = mention.menu();
        if items.is_empty() {
            return None;
        }
        let (point, line_height) = self.layouts.position(mention.at, Affinity::Downstream)?;
        let card = ui::menu::card(
            theme,
            "mention",
            &items,
            &mention.cursor,
            window,
            cx,
            |this: &mut Self, hit, _, cx| match hit {
                Hit::Point(path) => {
                    if let Some(mention) = this.mention.as_mut()
                        && mention.cursor.point_at(&mention.menu(), &path)
                    {
                        cx.notify();
                    }
                }
                Hit::Choose(path) => {
                    this.confirm_mention(path.first().copied(), cx);
                }
                Hit::Dismiss => {
                    this.mention = None;
                    cx.notify();
                }
            },
        )
        .max_h(px(280.0));
        Some(ui::popover::menu_at(
            "mention-menu",
            gpui::point(point.x, point.y + line_height),
            card.into_any_element(),
            None,
        ))
    }
}
