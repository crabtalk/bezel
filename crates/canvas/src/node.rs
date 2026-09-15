//! Who paints a node.
//!
//! An app's own kind — an article, a board, a live session — is a renderer
//! over its `type`, installed once at boot like `markdown`'s block renderer.
//! State stays with the app: a renderer looks its view up by the node's id and
//! hands the entity back.

use gpui::{AnyElement, App, Global, Window};

use crate::model::Node;

/// Paints a node's content at `zoom`, or `None` to leave it to the built-in
/// painting for its kind.
///
/// gpui has no transform for arbitrary elements, so zoom is a scale the
/// content applies itself — text through `Typography::scaled`, sizes by
/// multiplying. The box around it is already sized.
pub type NodeRenderer = fn(node: &Node, zoom: f32, &mut Window, &mut App) -> Option<AnyElement>;

struct Installed(NodeRenderer);

impl Global for Installed {}

/// `canvas::set_node_renderer(cx, my_nodes)` — call once at boot.
pub fn set_node_renderer(cx: &mut App, renderer: NodeRenderer) {
    cx.set_global(Installed(renderer));
}

pub(crate) fn render(
    node: &Node,
    zoom: f32,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyElement> {
    let renderer = cx.try_global::<Installed>()?.0;
    renderer(node, zoom, window, cx)
}
