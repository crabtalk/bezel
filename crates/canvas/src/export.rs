use crate::{CanvasEditor, CanvasView, clip};
use gpui::{App, Point, Size, Task, point, px, size};
use image::RgbaImage;
use std::{collections::HashMap, rc::Rc};

const PADDING: f32 = 32.;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Area {
    Canvas,
    Viewport,
}

pub struct Frame {
    pub size: Size<f32>,
    pub pan: Point<f32>,
    pub zoom: f32,
}

impl Area {
    pub fn frame(self, editor: &CanvasEditor) -> Result<Frame, String> {
        match self {
            Self::Viewport => Ok(Frame {
                size: editor.viewport().ok_or("canvas has not been laid out")?,
                pan: editor.pan(),
                zoom: editor.zoom(),
            }),
            Self::Canvas => {
                let ((x, y), (w, h)) =
                    clip::bounds(editor.canvas()).ok_or("canvas has no nodes")?;
                let (mut left, mut top, mut right, mut bottom) =
                    (x as f32, y as f32, (x + w) as f32, (y + h) as f32);
                for [x, y] in editor.canvas().edges.iter().flat_map(|edge| &edge.points) {
                    left = left.min(*x as f32);
                    top = top.min(*y as f32);
                    right = right.max(*x as f32);
                    bottom = bottom.max(*y as f32);
                }
                Ok(Frame {
                    size: size(right - left + PADDING * 2., bottom - top + PADDING * 2.),
                    pan: point(PADDING - left, PADDING - top),
                    zoom: 1.,
                })
            }
        }
    }
}

impl CanvasView {
    pub fn export_image(
        &self,
        area: Area,
        scale: f32,
        cx: &mut App,
    ) -> Result<Task<gpui::Result<RgbaImage>>, String> {
        let editor = self.editor();
        let frame = area.frame(editor)?;
        let mut pending = Vec::new();
        for node in &editor.canvas().nodes {
            if area == Area::Viewport {
                let left = node.x as f32 * frame.zoom + frame.pan.x;
                let top = node.y as f32 * frame.zoom + frame.pan.y;
                if left >= frame.size.width
                    || top >= frame.size.height
                    || left + node.width as f32 * frame.zoom <= 0.
                    || top + node.height as f32 * frame.zoom <= 0.
                {
                    continue;
                }
            }
            let kind = editor.kinds().get(&node.kind);
            if let Some(prepare) = &kind.prepare_export {
                pending.push((node.id.clone(), prepare(node, cx)));
            }
            if !editor.kinds().get(&node.kind).exportable {
                return Err(format!(
                    "node {} ({}) does not support image export",
                    node.id, node.kind
                ));
            }
        }
        let document = editor.canvas().clone();
        let mut kinds = editor.kinds().clone();
        let edges = editor.edge_kinds().clone();
        let layout = editor.layout();
        let mut options = editor.options();
        options.read_only = true;
        let (style, overlays) = (self.style, self.overlays.clone());
        Ok(cx.spawn(async move |cx| {
            let mut prepared = HashMap::new();
            for (id, task) in pending {
                let paint = task
                    .await
                    .map_err(|error| std::io::Error::other(format!("node {id}: {error}")))?;
                prepared.insert(id, paint);
            }
            let prepared = Rc::new(prepared);
            for node in &document.nodes {
                let original = kinds.get(&node.kind).clone();
                if original.prepare_export.is_none() {
                    continue;
                }
                let paint = original.render.clone();
                let prepared = prepared.clone();
                let mut kind = original;
                kind.prepare_export = None;
                kind.render = Rc::new(move |node, look, window, cx| {
                    prepared.get(&node.id).unwrap_or(&paint)(node, look, window, cx)
                });
                kinds = kinds.with(&node.kind, kind);
            }
            let task = cx.update(move |cx| {
                cx.render_view_to_image(frame.size.map(px), scale, move |_, cx| {
                    let mut view = CanvasView::new(document, layout, cx)
                        .with_kinds(kinds)
                        .with_edge_kinds(edges)
                        .with_options(options)
                        .with_style(style)
                        .with_overlays(overlays);
                    view.exporting = true;
                    view.update_editor(cx, |editor| {
                        editor.set_viewport(frame.size);
                        editor.set_zoom(frame.zoom);
                        let pan = editor.pan();
                        editor.pan_by(frame.pan.x - pan.x, frame.pan.y - pan.y);
                    });
                    view
                })
            })?;
            task.await
        }))
    }
}
