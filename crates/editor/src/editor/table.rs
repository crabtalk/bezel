//! Table movement shared by the handle drag and menu actions.

use super::*;
use gpui::AnyElement;

pub(super) struct TableDrag {
    pub block: usize,
    pub line: Line,
    pub start: gpui::Point<gpui::Pixels>,
    pub to: Option<usize>,
}

impl Editor {
    /// Move a body row to its final index, preserving selection and anchors.
    pub fn move_row(&mut self, block: usize, from: usize, to: usize, cx: &mut Context<Self>) {
        self.move_table_line(block, Line::Row(from), to, cx);
    }

    /// Move a column with its header and alignment.
    pub fn move_column(&mut self, block: usize, from: usize, to: usize, cx: &mut Context<Self>) {
        self.move_table_line(block, Line::Column(from), to, cx);
    }

    fn move_table_line(&mut self, block: usize, from: Line, to: usize, cx: &mut Context<Self>) {
        if !self.blocks() {
            return;
        }
        let Some(BlockKind::Table { header, rows, .. }) =
            self.doc.blocks.get(block).map(|b| &b.kind)
        else {
            return;
        };
        let valid = match from {
            Line::Row(from) => {
                from != to && from > 0 && to > 0 && from <= rows.len() && to <= rows.len()
            }
            Line::Column(from) => {
                let width = rows
                    .iter()
                    .map(Vec::len)
                    .chain([header.len()])
                    .max()
                    .unwrap_or(0);
                from != to && from < width && to < width
            }
        };
        if !valid {
            return;
        }
        self.edit(EditKind::Structure, cx, |this| {
            match from {
                Line::Row(from) => this.doc.move_row(block, from, to),
                Line::Column(from) => this.doc.move_column(block, from, to),
            };
            let delta = Delta::CellsMoved { block, from, to };
            this.selection = Selection::new(
                delta.cursor(this.selection.anchor).unwrap(),
                delta.cursor(this.selection.head).unwrap(),
            );
            this.hovered_cell = None;
            vec![delta]
        });
    }

    pub(super) fn drag_table_to(
        &mut self,
        position: gpui::Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = &mut self.table_drag else {
            return;
        };
        if (position - drag.start).magnitude() > 4.0 {
            self.table_dragged = true;
            self.table_menu.close();
        }
        if !self.table_dragged {
            return;
        }
        drag.to = self
            .layouts
            .block_bounds(drag.block)
            .filter(|bounds| bounds.contains(&position))
            .and_then(|_| self.doc.blocks.get(drag.block))
            .and_then(|block| {
                block.parts().into_iter().find_map(|part| {
                    let bounds = self.layouts.cell_bounds(drag.block, part)?;
                    match (drag.line, part) {
                        (Line::Row(from), Part::Cell { row, .. })
                            if from > 0
                                && row > 0
                                && position.y >= bounds.top()
                                && position.y <= bounds.bottom() =>
                        {
                            Some(row)
                        }
                        (Line::Column(_), Part::Cell { column, .. })
                            if position.x >= bounds.left() && position.x <= bounds.right() =>
                        {
                            Some(column)
                        }
                        _ => None,
                    }
                })
            });
        cx.notify();
    }

    pub(super) fn drop_table_drag(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(drag) = self.table_drag.take() else {
            return false;
        };
        if self.table_dragged
            && let Some(to) = drag.to
        {
            self.move_table_line(drag.block, drag.line, to, cx);
        }
        cx.notify();
        true
    }

    pub(super) fn table_drop_indicator(&self, theme: &theme::Theme) -> Option<AnyElement> {
        let drag = self.table_drag.as_ref()?;
        let to = drag.to?;
        let table = self.layouts.block_bounds(drag.block)?;
        let lane = gpui::px(markdown::render::TABLE_CONTROL_SIZE);
        let (part, after) = match drag.line {
            Line::Row(from) if from != to => (Part::Cell { row: to, column: 0 }, to > from),
            Line::Column(from) if from != to => {
                let row =
                    self.doc.blocks[drag.block]
                        .parts()
                        .first()
                        .and_then(|part| match part {
                            Part::Cell { row, .. } => Some(*row),
                            _ => None,
                        })?;
                (Part::Cell { row, column: to }, to > from)
            }
            _ => return None,
        };
        let cell = self.layouts.cell_bounds(drag.block, part)?;
        let mark = div()
            .absolute()
            .bg(theme.drop_line)
            .debug_selector(|| "table-drop".into());
        Some(
            match drag.line {
                Line::Row(_) => mark
                    .left(table.left() - self.origin.x + lane)
                    .top(if after { cell.bottom() } else { cell.top() } - self.origin.y)
                    .w(table.size.width - lane * 2.0)
                    .h(gpui::px(1.0)),
                Line::Column(_) => mark
                    .left(if after { cell.right() } else { cell.left() } - self.origin.x)
                    .top(table.top() - self.origin.y + lane)
                    .w(gpui::px(1.0))
                    .h(table.size.height - lane * 2.0),
            }
            .into_any_element(),
        )
    }
}
