//! A canvas drawn whole for reading: one canvas unit to the pixel, no pan, no
//! zoom, nothing to pick. Node text is set plain, centred in its shape.

use gpui::{App, Bounds, Div, Hsla, SharedString, div, point, prelude::*, px, size};
use theme::{TextStyle, Theme, Typeset};

use crate::{
    model::{Canvas, End, GROUP, Node},
    paint,
    path::{self, Ends, Path, Rect},
};

/// Room kept around what the canvas holds.
const MARGIN: f32 = 8.0;
/// A box's corners.
const RADIUS: f32 = 6.0;
/// Inside a node, between its outline and its text.
const PAD: f32 = 8.0;
/// An end's mark, across.
const MARK: f32 = 8.0;
/// A group's wash of its colour.
const GROUP_WASH: f32 = 0.04;
/// A node's wash of ink, over what the diagram sits on.
const NODE_WASH: f32 = 0.06;

/// `canvas` at its own size: every node, group and edge, with its labels.
pub fn diagram(canvas: &Canvas, cx: &App) -> Div {
    let theme = Theme::of(cx).clone();
    let Some((x0, y0, x1, y1)) = extent(canvas) else {
        return div();
    };
    let shift = (MARGIN - x0, MARGIN - y0);
    let (width, height) = (x1 - x0 + 2.0 * MARGIN, y1 - y0 + 2.0 * MARGIN);
    let place = move |x: f32, y: f32| (x + shift.0, y + shift.1);

    let nodes = canvas.lookup();
    let paths: Vec<(Path, crate::model::Edge)> = canvas
        .edges
        .iter()
        .filter_map(|edge| Some((route(&nodes, edge)?, edge.clone())))
        .collect();
    let tint = |node: &Node| node.color.as_deref().and_then(|c| paint::color(&theme, c));

    let (groups, plain): (Vec<&Node>, Vec<&Node>) =
        canvas.nodes.iter().partition(|node| node.kind == GROUP);
    let boxes = |nodes: &[&Node]| -> Vec<(Bounds<f32>, crate::model::Shape, Hsla, Hsla)> {
        nodes
            .iter()
            .map(|node| {
                let (x, y) = place(node.x as f32, node.y as f32);
                let bounds = Bounds::new(point(x, y), size(node.width as f32, node.height as f32));
                let border = tint(node).unwrap_or(theme.border);
                let fill = match node.kind == GROUP {
                    true => tint(node).map_or(gpui::transparent_black(), |c| c.opacity(GROUP_WASH)),
                    false => theme.ink(NODE_WASH),
                };
                (bounds, node.shape.unwrap_or_default(), fill, border)
            })
            .collect()
    };
    let (group_boxes, node_boxes) = (boxes(&groups), boxes(&plain));
    let strokes: Vec<(Path, crate::model::Stroke, Hsla)> = paths
        .iter()
        .map(|(path, edge)| {
            let color = edge
                .color
                .as_deref()
                .and_then(|c| paint::color(&theme, c))
                .unwrap_or(theme.border_strong);
            (path.clone(), edge.style.unwrap_or_default(), color)
        })
        .collect();

    let painter = gpui::canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let origin = (bounds.origin.x.as_f32(), bounds.origin.y.as_f32());
            let on_screen = |b: &Bounds<f32>| {
                Bounds::new(
                    point(px(origin.0 + b.origin.x), px(origin.1 + b.origin.y)),
                    size(px(b.size.width), px(b.size.height)),
                )
            };
            for (b, shape, fill, border) in &group_boxes {
                paint::shape(window, *shape, on_screen(b), RADIUS, *fill, *border);
            }
            let screen =
                |p: gpui::Point<f32>| point(origin.0 + p.x + shift.0, origin.1 + p.y + shift.1);
            for (path, stroke, color) in &strokes {
                paint::edge(window, path, screen, *stroke, 1.0, MARK, *color);
            }
            for (b, shape, fill, border) in &node_boxes {
                paint::shape(window, *shape, on_screen(b), RADIUS, *fill, *border);
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full();

    let label = |text: SharedString| {
        div()
            .text_style(TextStyle::Callout)
            .text_color(theme.text)
            .child(text)
    };
    let node_labels = plain.iter().filter_map(|node| {
        let text = node.text.clone().or_else(|| node.label.clone())?;
        let (x, y) = place(node.x as f32, node.y as f32);
        Some(
            div()
                .absolute()
                .left(px(x))
                .top(px(y))
                .w(px(node.width as f32))
                .h(px(node.height as f32))
                .p(px(PAD))
                .flex()
                .items_center()
                .justify_center()
                .text_center()
                .overflow_hidden()
                .child(label(text.into())),
        )
    });
    let group_labels = groups.iter().filter_map(|node| {
        let text = node.label.clone()?;
        let (x, y) = place(node.x as f32, node.y as f32);
        Some(
            div()
                .absolute()
                .left(px(x + PAD))
                .top(px(y + PAD / 2.0))
                .text_style(TextStyle::Caption)
                .text_color(theme.text_muted)
                .child(text),
        )
    });
    let edge_labels = paths.iter().filter_map(|(path, edge)| {
        let text = edge.label.clone()?;
        let at = path.middle();
        let (x, y) = place(at.x, at.y);
        // Centred on the point by a zero-size anchor its child hangs across.
        Some(
            div().absolute().left(px(x)).top(px(y)).size_0().child(
                div()
                    .absolute()
                    .flex()
                    .justify_center()
                    .items_center()
                    .left(px(-60.0))
                    .w(px(120.0))
                    .top(px(-10.0))
                    .h(px(20.0))
                    .child(
                        div()
                            .px(px(4.0))
                            .rounded(px(4.0))
                            .bg(theme.surface_card)
                            .text_style(TextStyle::Caption)
                            .text_color(theme.text_muted)
                            .child(text),
                    ),
            ),
        )
    });

    div()
        .relative()
        .flex_none()
        .w(px(width))
        .h(px(height))
        .child(painter)
        .children(group_labels)
        .children(node_labels)
        .children(edge_labels)
}

/// The width and height [`diagram`] draws `canvas` at, or `None` for a canvas
/// with nothing in it.
pub fn size_of(canvas: &Canvas) -> Option<(f32, f32)> {
    let (x0, y0, x1, y1) = extent(canvas)?;
    Some((x1 - x0 + 2.0 * MARGIN, y1 - y0 + 2.0 * MARGIN))
}

/// Where `edge` runs: its route, else the spec's curve between its boxes.
fn route(
    nodes: &std::collections::HashMap<&str, &Node>,
    edge: &crate::model::Edge,
) -> Option<Path> {
    let rect = |id: &str| {
        nodes
            .get(id)
            .map(|node| Rect::of(node, (node.x as f32, node.y as f32)))
    };
    let ends = Ends {
        from_side: edge.from_side,
        to_side: edge.to_side,
        from_end: edge.from_end.unwrap_or(End::None),
        to_end: edge.to_end.unwrap_or(End::Arrow),
        ..Ends::between(rect(&edge.from_node)?, rect(&edge.to_node)?)
    };
    Some(path::route(&ends, &edge.points).unwrap_or_else(|| path::curve(&ends)))
}

/// What the nodes and routes cover: left, top, right, bottom.
fn extent(canvas: &Canvas) -> Option<(f32, f32, f32, f32)> {
    let corners = canvas
        .nodes
        .iter()
        .flat_map(|node| {
            let (x, y) = (node.x as f32, node.y as f32);
            [(x, y), (x + node.width as f32, y + node.height as f32)]
        })
        .chain(
            canvas
                .edges
                .iter()
                .flat_map(|edge| edge.points.iter().map(|&[x, y]| (x as f32, y as f32))),
        );
    corners.fold(None, |hull, (x, y)| {
        Some(match hull {
            None => (x, y, x, y),
            Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
        })
    })
}
