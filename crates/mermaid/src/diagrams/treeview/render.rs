//! Pele's `diagrams/treeview/render.ts`.

use super::TreeViewConfig;
use crate::config::within;
use crate::geom::Point;
use crate::svg::builder::{Esc, EscText};
use std::{collections::HashMap, fmt::Write as _};

use super::{
    icons::{BUILTIN_PACK, IconConfig, glyph},
    model::{NodeType, TreeNode, TreeViewModel},
};
use crate::{Config, Ctx, Error, Host, util::chars};
use crate::{
    Rendered,
    diagrams::Render,
    js,
    svg::{
        builder::{TITLE, fragment, icon_element, num},
        root::svg_document,
        theme::{RADIUS, class_names},
    },
    text::{Label, Style, decode_entities},
};

const ICON_GAP: f64 = 6.0;
const DESC_GAP: f64 = 16.0;
const GUIDE_INSET: f64 = 7.0;
const ELBOW: f64 = 4.0;
const HIGHLIGHT_OVERHANG: f64 = 4.0;

/// Mermaid's layout ctx, with Pele's defaults.
struct TreeSettings {
    padding_x: f64,
    padding_y: f64,
    row_indent: f64,
    line_thickness: f64,
}

impl TreeSettings {
    fn new(config: &TreeViewConfig) -> Self {
        Self {
            padding_x: within(config.padding_x, 5.0, 0.0, 100.0),
            padding_y: within(config.padding_y, 2.0, 0.0, 100.0),
            row_indent: within(config.row_indent, 15.0, 0.0, 400.0),
            line_thickness: within(config.line_thickness, 1.0, 0.25, 12.0),
        }
    }
}

struct Row<'m> {
    node: &'m TreeNode,
    /// Where the row starts at its depth, on its middle line, before the
    /// drawing is offset.
    start: Point,
    icon: Option<String>,
    text: String,
    label_x: f64,
    highlighted: bool,
    label: Option<Label>,
    description: Option<Label>,
    top: f64,
    height: f64,
}

impl Render for TreeViewModel {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        Ok(ctx.tighten(|tight| self.draw(&config.tree_view, ctx, tight)))
    }
}

impl TreeNode {
    fn label_style(&self) -> u8 {
        match self.node_type {
            NodeType::Directory => Style::BOLD,
            NodeType::File => 0,
        }
    }
}

impl TreeViewModel {
    fn draw(&self, config: &TreeViewConfig, ctx: &Ctx<'_, impl Host>, tight: bool) -> Rendered {
        let p = crate::PREFIX;
        let size = ctx.options.font_size;
        let measure = ctx.host;
        let pad = ctx.options.padding.unwrap_or(8.0);
        let icons = IconConfig::new(config);
        let max_width = ctx.options.max_width.unwrap_or(320.0);

        let TreeSettings {
            padding_x,
            padding_y,
            row_indent,
            line_thickness,
        } = TreeSettings::new(config);
        let mut max_depth = 0.0f64;
        let mut depths: Vec<(&TreeNode, f64)> =
            self.root.children.iter().map(|node| (node, 0.0)).collect();
        let mut i = 0;
        while tight && i < depths.len() {
            let (node, depth) = depths[i];
            max_depth = max_depth.max(depth);
            for child in &node.children {
                depths.push((child, depth + 1.0));
            }
            i += 1;
        }
        let step = if tight {
            (row_indent + padding_x).min(2f64.max((max_width - 120.0) / 1f64.max(max_depth)))
        } else {
            row_indent + padding_x
        };
        let row_height = js::round(size * 1.5) + 2.0 * padding_y;
        let icon_size = size;

        let title = Label::layout(
            self.titles.title.as_deref(),
            false,
            measure,
            size,
            if tight { ctx.title_room() } else { 4000.0 },
            Style::BOLD,
        );
        let title_height = title.spaced_height(12.0);

        // Siblings line up: when one entry of a folder has an icon, the others
        // leave room for one.
        let has_icon = |list: &[TreeNode]| list.iter().any(|child| icons.of(child).is_some());
        let mut rows: Vec<Row> = Vec::new();
        let mut row_of: HashMap<usize, usize> = HashMap::new();
        let mut pending: Vec<(&TreeNode, f64, bool)> = Vec::new();
        fn push<'m>(
            pending: &mut Vec<(&'m TreeNode, f64, bool)>,
            list: &'m [TreeNode],
            depth: f64,
            slot: bool,
        ) {
            for node in list.iter().rev() {
                pending.push((node, depth, slot));
            }
        }
        push(
            &mut pending,
            &self.root.children,
            0.0,
            has_icon(&self.root.children),
        );
        let mut label_right = 0.0;
        let mut any_highlight = false;
        let mut rows_height = 0.0;
        while let Some((node, depth, slot)) = pending.pop() {
            let x = depth * step;
            let text = decode_entities(&node.name);
            let label_x = x + if slot { icon_size + ICON_GAP } else { 0.0 };
            let room = 40f64.max(max_width - 2.0 * pad - 2.0 * HIGHLIGHT_OVERHANG - label_x);
            let label =
                tight.then(|| Label::literal(&text, measure, size, room, node.label_style()));
            let description = match &node.description {
                Some(description) if tight && !description.is_empty() => Some(Label::literal(
                    &decode_entities(description),
                    measure,
                    size,
                    room,
                    Style::ITALIC,
                )),
                _ => None,
            };
            let width = match &label {
                Some(label) => label.width,
                None => measure.width(&text, size, node.label_style()),
            };
            if label_x + width > label_right {
                label_right = label_x + width;
            }
            let highlighted = node
                .css_class
                .as_deref()
                .is_some_and(|class| class.split(chars::is_space).any(|name| name == "highlight"));
            if highlighted {
                any_highlight = true;
            }
            let label_height = label.as_ref().map_or(0.0, |label| label.height);
            let height = if tight {
                row_height.max(
                    label_height
                        + description
                            .as_ref()
                            .map_or(0.0, |description| description.height)
                        + 2.0 * padding_y,
                )
            } else {
                row_height
            };
            let cy = rows_height
                + if tight {
                    padding_y + icon_size.max(label_height) / 2.0
                } else {
                    height / 2.0
                };
            row_of.insert(node.id, rows.len());
            rows.push(Row {
                node,
                start: Point::new(x, cy),
                icon: icons.of(node),
                text,
                label_x,
                highlighted,
                label,
                description,
                top: rows_height,
                height,
            });
            rows_height += height;
            push(
                &mut pending,
                &node.children,
                depth + 1.0,
                has_icon(&node.children),
            );
        }

        let desc_x = label_right + DESC_GAP;
        let mut width = label_right;
        for row in &rows {
            let Some(description) = row.node.description.as_deref().filter(|d| !d.is_empty())
            else {
                continue;
            };
            let w = match &row.description {
                Some(label) => label.width,
                None => measure.width(&decode_entities(description), size, Style::ITALIC),
            };
            width = width.max(if tight { row.label_x } else { desc_x } + w);
        }
        width = width.max(title.width);
        let inset = pad
            + if any_highlight {
                HIGHLIGHT_OVERHANG
            } else {
                0.0
            };
        let top = pad + title_height;
        let total_width = (width + 2.0 * inset).ceil();
        let total_height = (top + rows_height + pad).ceil();

        let cy_of = |node: &TreeNode| rows[row_of[&node.id]].start.y;
        let mut guides = String::new();
        for row in &rows {
            let children = &row.node.children;
            let Some(last_child) = children.last() else {
                continue;
            };
            let gx = inset + row.start.x + GUIDE_INSET;
            let tick = gx.max(inset + row.start.x + step - padding_x);
            let last = top + cy_of(last_child);
            let r = ELBOW.min(tick - gx);
            let d = &mut guides;
            let _ = write!(
                d,
                "<path d=\"M{},{}V{}",
                num(gx),
                num(top + row.top + row.height - 2.0),
                num(last - r)
            );
            if r > 0.0 {
                let _ = write!(
                    d,
                    "Q{},{} {},{}H{}",
                    num(gx),
                    num(last),
                    num(gx + r),
                    num(last),
                    num(tick)
                );
            }
            if tick > gx {
                for child in &children[..children.len() - 1] {
                    let _ = write!(d, "M{},{}H{}", num(gx), num(top + cy_of(child)), num(tick));
                }
            }
            d.push_str("\"/>");
        }

        let mut content = String::with_capacity(guides.len() + 16 * 1024);
        let _ = write!(
            content,
            "{}",
            title.svg(
                Point::new(total_width / 2.0, pad + title.height / 2.0),
                TITLE
            )
        );
        if !guides.is_empty() {
            let _ = write!(
                content,
                "<g class=\"{p}-tree-guides\" fill=\"none\" stroke=\"var(--_b)\" stroke-width=\"{}\">{guides}</g>",
                num(line_thickness)
            );
        }
        let baseline = size * 0.35;
        for row in &rows {
            let node = row.node;
            let cy = top + row.start.y;
            let directory = node.node_type == NodeType::Directory;
            let weight = if directory {
                " font-weight=\"var(--_hw)\""
            } else {
                ""
            };
            let label_attrs = if directory {
                crate::class!("label", "font-weight=\"var(--_hw)\"")
            } else {
                crate::class!("label")
            };
            let _ = write!(
                content,
                "<g class=\"{p}-node {p}-tree-{}{}\" data-id=\"{}\">",
                if directory { "dir" } else { "file" },
                class_names(node.css_class.as_deref().unwrap_or("")),
                EscText(&node.name)
            );
            let inner = &mut content;
            if row.highlighted {
                let x = inset + row.start.x - HIGHLIGHT_OVERHANG;
                let _ = write!(
                    inner,
                    "<rect class=\"{p}-tree-highlight\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_c)\" fill-opacity=\"0.14\"/>",
                    num(x),
                    num(top + row.top + 1.0),
                    num(total_width - inset + HIGHLIGHT_OVERHANG - x),
                    num(row.height - 2.0)
                );
            }
            if let Some(icon) = &row.icon {
                let name = icon
                    .strip_prefix(BUILTIN_PACK)
                    .and_then(|rest| rest.strip_prefix(':'))
                    .unwrap_or("");
                let glyph = glyph(name)
                    .or_else(|| glyph(if directory { "folder" } else { "file" }))
                    .expect("a built-in glyph");
                let drawn = fragment(|f| match crate::text::icon(ctx.host, icon) {
                    Some(markup) => f.write_str(&markup),
                    None => write!(
                        f,
                        "<path d=\"{glyph}\" stroke=\"var(--_m)\" stroke-width=\"1.5\"/>"
                    ),
                });
                let _ = write!(
                    inner,
                    "{}",
                    icon_element(
                        icon,
                        Point::new(inset + row.start.x, cy - icon_size / 2.0),
                        icon_size,
                        drawn
                    )
                );
            }
            if let Some(label) = &row.label {
                let _ = write!(
                    inner,
                    "{}",
                    label.start_lines(
                        Point::new(inset + row.label_x, top + row.top + padding_y),
                        label_attrs,
                    )
                );
            } else if !row.text.is_empty() {
                let _ = write!(
                    inner,
                    "<text class=\"{p}-label\" x=\"{}\" y=\"{}\"{weight} xml:space=\"preserve\">{}</text>",
                    num(inset + row.label_x),
                    num(cy + baseline),
                    Esc(&row.text)
                );
            }
            if let Some(description) = &row.description {
                let _ = write!(
                    inner,
                    "{}",
                    description.start_lines(
                        Point::new(
                            inset + row.label_x,
                            top + row.top
                                + padding_y
                                + row.label.as_ref().map_or(0.0, |label| label.height),
                        ),
                        crate::class!(
                            "tree-description",
                            "font-style=\"italic\" fill=\"var(--_m)\""
                        ),
                    )
                );
            } else if let Some(description) = node.description.as_deref().filter(|d| !d.is_empty())
            {
                let _ = write!(
                    inner,
                    "<text class=\"{p}-tree-description\" x=\"{}\" y=\"{}\" font-style=\"italic\" fill=\"var(--_m)\" xml:space=\"preserve\">{}</text>",
                    num(inset + desc_x),
                    num(cy + baseline),
                    Esc(&decode_entities(description))
                );
            }
            content.push_str("</g>");
        }

        Rendered {
            svg: svg_document(
                "treeView",
                total_width,
                total_height,
                size,
                ctx,
                self.titles.accessible(),
                &content,
            ),
            width: total_width,
            height: total_height,
            links: Vec::new(),
        }
    }
}
