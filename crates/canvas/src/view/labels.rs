use crate::view::*;
use gpui::Font;
use markdown::Typography;

#[derive(PartialEq)]
struct Key {
    text: String,
    typography: Typography,
    font: Font,
    zoom: f32,
    size: (f32, f32),
    scale: f32,
}

pub(super) struct Layout {
    key: Key,
    bounds: Rect,
}

impl Layout {
    fn visible(&self, center: (f32, f32), viewport: gpui::Size<Pixels>) -> bool {
        let (x, y) = (center.0 + self.bounds.x, center.1 + self.bounds.y);
        x <= viewport.width.as_f32()
            && y <= viewport.height.as_f32()
            && x + self.bounds.w >= 0.
            && y + self.bounds.h >= 0.
    }
}

impl CanvasView {
    /// Edge labels, at the middle of their curves, and the one being typed.
    pub(super) fn labels(
        &self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let (z, pan) = (self.editor.zoom(), self.editor.pan());
        let canvas = self.editor.painted();
        self.label_layouts
            .borrow_mut()
            .retain(|id, _| self.geometry.paths.get(id).is_some());
        canvas
            .edges
            .iter()
            .filter_map(|edge| {
                let editing = self
                    .editing
                    .as_ref()
                    .filter(|s| s.edge_id() == Some(edge.id.as_str()));
                let text = self
                    .editor
                    .edge_kinds()
                    .get(edge)
                    .rules
                    .edit
                    .as_ref()
                    .map(|field| (field.read)(edge))
                    .filter(|label| !label.is_empty());
                let label_scale = edge.text_size() / model::TEXT_SIZE;
                if editing.is_none() && text.is_none() {
                    return None;
                }
                let middle = self.geometry.paths.get(&edge.id)?.middle();
                let (x, y) = (pan.x + middle.x * z, pan.y + middle.y * z);
                let typography = presentation::text_typography(cx, edge.text_size(), z);
                let key = Key {
                    text: text.clone().unwrap_or_default(),
                    typography,
                    font: window.text_style().font(),
                    zoom: z,
                    size: self.style.label,
                    scale: label_scale,
                };
                if editing.is_none()
                    && let Some(viewport) = self.viewport.get()
                    && let Some(layout) = self.label_layouts.borrow().get(&edge.id)
                    && layout.key == key
                    && !layout.visible((x, y), viewport.size)
                {
                    return None;
                }
                let report = if editing.is_none() {
                    self.viewport.get().map(|viewport| {
                        let layouts = self.label_layouts.clone();
                        let id = edge.id.clone();
                        painter(
                            move |bounds, _, _| {
                                layouts.borrow_mut().insert(
                                    id,
                                    Layout {
                                        key,
                                        bounds: Rect {
                                            x: bounds.origin.x.as_f32()
                                                - viewport.origin.x.as_f32()
                                                - x,
                                            y: bounds.origin.y.as_f32()
                                                - viewport.origin.y.as_f32()
                                                - y,
                                            w: bounds.size.width.as_f32(),
                                            h: bounds.size.height.as_f32(),
                                        },
                                    },
                                );
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full()
                    })
                } else {
                    None
                };
                let body = match editing {
                    Some(session) => div()
                        .min_w(px(self.style.label.0 / 2.0 * z * label_scale))
                        .children(session.input.inline())
                        .into_any_element(),
                    None => div()
                        .text_size(px(typography.body.size()))
                        .line_height(px(typography.body.line_height()))
                        .text_color(theme.text)
                        .child(text.unwrap_or_default())
                        .into_any_element(),
                };
                let picked = self.editor.selected_edge() == Some(edge.id.as_str());
                let id = edge.id.clone();
                let label = div()
                    .id(ElementId::Name(format!("edge-label-{}", edge.id).into()))
                    .px(px(PAD / 2.0 * z))
                    .rounded(px(RADIUS * z))
                    .border_1()
                    .border_color(if picked { theme.ring } else { theme.border })
                    .bg(theme.surface_card)
                    .relative()
                    .child(body)
                    .children(report)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            this.press(event, Hit::Edge(id.clone()), window, cx);
                        }),
                    );
                Some(
                    div()
                        .absolute()
                        .left(px(x - self.style.label.0 * z * label_scale / 2.0))
                        .top(px(y - self.style.label.1 * z * label_scale / 2.0))
                        .w(px(self.style.label.0 * z * label_scale))
                        .h(px(self.style.label.1 * z * label_scale))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(label)
                        .into_any_element(),
                )
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "../../tests/unit/labels.rs"]
mod tests;
