//! Tables.

use super::*;

/// A GFM table.
///
/// Columns are content-proportional with a per-column floor: each cell is
/// shaped unwrapped to get its max-content width, and the flex resolution does
/// the rest. When even the floors no longer fit, the table scrolls sideways
/// rather than crushing every column into per-character wrapping.
#[expect(
    clippy::too_many_arguments,
    reason = "a table, its overlay, and what paints them"
)]
pub(super) fn table(
    align: &[Align],
    header: &[Text],
    rows: &[Vec<Text>],
    overlay: Overlay,
    typography: &Typography,
    theme: &Theme,
    window: &mut Window,
    cx: &App,
) -> AnyElement {
    let ix = overlay.block;
    let all: Vec<&[Text]> = std::iter::once(header)
        .filter(|row| !row.is_empty())
        .chain(rows.iter().map(|row| row.as_slice()))
        .collect();
    let columns = all.iter().map(|row| row.len()).max().unwrap_or(0);
    if columns == 0 {
        return gpui::Empty.into_any_element();
    }
    let has_header = !header.is_empty();

    let text_system = window.text_system();
    let mut flats: Vec<Vec<Option<Flat>>> = Vec::with_capacity(all.len());
    let mut content = vec![0.0f32; columns];
    for (r, row) in all.iter().enumerate() {
        let weight = if has_header && r == 0 {
            FontWeight::BOLD
        } else {
            FontWeight::NORMAL
        };
        let mut out = Vec::with_capacity(columns);
        for (c, natural) in content.iter_mut().enumerate() {
            let Some(cell) = row.get(c) else {
                out.push(None);
                continue;
            };
            let flat = flatten_with(cell, weight, theme, |name| {
                crate::marks::paint_of(cx, name, theme)
            });
            if !flat.text.is_empty() {
                let width = f32::from(
                    text_system
                        .shape_line(
                            flat.text.clone(),
                            px(typography.body.size()),
                            &flat.runs,
                            None,
                        )
                        .width(),
                );
                *natural = natural.max(width);
            }
            out.push(Some(flat));
        }
        flats.push(out);
    }

    let naturals: Vec<f32> = content
        .iter()
        .map(|width| width.max(TABLE_MIN_COLUMN_CONTENT) + 2.0 * TABLE_CELL_PADDING)
        .collect();
    let minimums: Vec<f32> = naturals
        .iter()
        .map(|natural| natural.min(TABLE_MIN_COLUMN_WIDTH))
        .collect();
    let hairline = theme.hairline(0.10);

    let mut inner = div()
        .flex()
        .flex_col()
        .w_full()
        .min_w(px(minimums.iter().sum::<f32>()));
    for (r, row) in flats.into_iter().enumerate() {
        if r > 0 {
            inner = inner.child(div().flex_none().h(px(TABLE_DIVIDER)).w_full().bg(hairline));
        }
        let mut row_el = div().flex().flex_row();
        for (c, cell) in row.into_iter().enumerate() {
            let mut cell_el = div()
                .flex_grow(naturals[c])
                .flex_shrink(naturals[c])
                .flex_basis(px(0.0))
                .min_w(px(minimums[c]))
                .p(px(TABLE_CELL_PADDING))
                .text_size(px(typography.body.size()))
                .line_height(px(typography.body.line_height()));
            cell_el = match align.get(c).copied().unwrap_or_default() {
                Align::Left => cell_el,
                Align::Center => cell_el.text_center(),
                Align::Right => cell_el.text_right(),
            };
            if let Some(flat) = cell {
                // `all` drops an empty header, so a table without one starts at
                // part row 1 — row 0 is the header slot whether or not it is
                // filled.
                let row = if has_header { r } else { r + 1 };
                let len = flat.text.len();
                cell_el = cell_el.child(painted_text(
                    flat,
                    len,
                    typography.body.size(),
                    typography.body.line_height(),
                    overlay.at(Part::Cell { row, column: c }),
                    theme,
                ));
            }
            row_el = row_el.child(cell_el);
        }
        inner = inner.child(row_el);
    }

    ui::scroll::Viewport::new(
        format!("md-table-scroll-{ix}"),
        div()
            .id(ElementId::named_usize("md-table", ix))
            .w_full()
            .child(inner),
        gpui::Axis::Horizontal,
    )
    .into_any_element()
}
