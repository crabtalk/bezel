//! Node positions, node and label painting, and the edge layer.

use super::*;

impl CanvasView {
    /// Where each node paints this frame. A node the document moved glides
    /// there from where it was painted; the held node follows the pointer, and
    /// a node never painted yet starts where it is.
    pub(super) fn positions(&mut self, reduced: bool, window: &mut Window) -> Positions {
        let held = self.held().map(str::to_owned);
        let fixed =
            |id: &str, shown: &Positions| held.as_deref() == Some(id) || !shown.contains_key(id);
        let canvas = self.editor.painted();
        let to: Positions = canvas
            .nodes
            .iter()
            .filter(|n| !fixed(&n.id, &self.shown))
            .map(|n| (n.id.clone(), (n.x as f32, n.y as f32)))
            .collect();
        let moved = to.iter().any(|(id, at)| self.shown.get(id) != Some(at));
        if reduced {
            self.glide = None;
        } else if moved && self.glide.as_ref().is_none_or(|glide| glide.to != to) {
            self.glide = Some(Glide {
                from: self.shown.clone(),
                to,
                since: Instant::now(),
            });
        }

        let raw = self.glide.as_ref().map_or(1.0, |glide| {
            let total = LAYOUT.total().mul_f32(motion::speed_scale());
            glide.since.elapsed().as_secs_f32() / total.as_secs_f32().max(f32::EPSILON)
        });
        let t = LAYOUT.progress(raw);
        let shown: Positions = canvas
            .nodes
            .iter()
            .map(|n| {
                let at = (n.x as f32, n.y as f32);
                let from = self
                    .glide
                    .as_ref()
                    .filter(|_| !fixed(&n.id, &self.shown))
                    .and_then(|glide| glide.from.get(&n.id));
                let at = from.map_or(at, |&from| {
                    (motion::lerp(from.0, at.0, t), motion::lerp(from.1, at.1, t))
                });
                (n.id.clone(), at)
            })
            .collect();
        if raw >= 1.0 {
            self.glide = None;
        } else {
            window.request_animation_frame();
        }
        self.shown = shown.clone();
        shown
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn paint_node(
        &self,
        ix: usize,
        at: (f32, f32),
        connecting: Option<&str>,
        picked: &[&str],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let node = &self.editor.painted().nodes[ix];
        let (z, pan) = (self.editor.zoom(), self.editor.pan());
        let (x, y) = (pan.x + at.0 * z, pan.y + at.1 * z);
        let (w, h) = (node.width as f32 * z, node.height as f32 * z);
        if let Some(viewport) = self.viewport.get() {
            let (vw, vh) = (viewport.size.width.as_f32(), viewport.size.height.as_f32());
            if x > vw || y > vh || x + w < 0.0 || y + h < 0.0 {
                return None;
            }
        }
        let kind = self.editor.kinds().get(&node.kind);
        let editing = self.editing.as_ref().filter(|s| !s.edge && s.id == node.id);
        // A node being typed in keeps the editor's own cursor.
        let draggable = editing.is_none();
        // Far out a node is a plain box, its content too small to read.
        let far = z < self.style.far_zoom && draggable;
        let mark = Mark {
            width: w,
            height: h,
            zoom: z,
            style: self.style,
        };
        let content = if far {
            (self.overlays.placeholder)(&mark, window, cx)
        } else {
            let look = Look {
                zoom: z,
                editor: editing.map(|session| session.input.clone().into_any_element()),
            };
            (kind.render)(node, look, window, cx)
        };
        let grows = kind.rules.sizing == Sizing::Grows && !far;
        let selected = picked.contains(&node.id.as_str());
        // A drop, or a connector let go here, would connect to this node.
        let target = connecting == Some(node.id.as_str())
            || self.held() != Some(node.id.as_str())
                && self.editor.pending.iter().any(|change| {
                    matches!(change, Change::AddEdge { edge, .. }
                        if edge.from_node == node.id || edge.to_node == node.id)
                });
        // A node picked alone, with nothing held, shows the handles its kind
        // declares, less what it will not let the reader do.
        let shows = selected && draggable && picked.len() == 1 && self.holding.is_none();
        let allowed = |role: Role| match role {
            Role::Connect => self.editor.node_can(&node.id, Capability::Connectable),
            Role::Resize => self.editor.node_can(&node.id, Capability::Resizable),
            Role::Reconnect(_) => false,
        };
        let declared: Vec<Handle> = match shows {
            true => (kind.rules.handles)(node)
                .into_iter()
                .filter(|declared| allowed(declared.role))
                .collect(),
            false => Vec::new(),
        };
        let (id, measured_id) = (node.id.clone(), node.id.clone());
        let (measured, height) = (self.measured.clone(), node.height);
        let wash = target.then(|| (self.overlays.drop)(&mark, window, cx));
        let ring = (selected || target).then(|| (self.overlays.ring)(&mark, window, cx));
        let handles: Vec<AnyElement> = declared
            .into_iter()
            .map(|declared| {
                let (spot, _) = declared
                    .spot
                    .on(Rect::of(node, at))
                    .unwrap_or((point(0.0, 0.0), point(0.0, 0.0)));
                let (hx, hy) = ((spot.x - at.0) * z, (spot.y - at.1) * z);
                let size = self.style.handle;
                let look = (self.overlays.handle)(
                    &Mark {
                        width: size,
                        height: size,
                        ..mark
                    },
                    window,
                    cx,
                );
                let cursor = match declared.role {
                    Role::Resize => CursorStyle::ResizeUpLeftDownRight,
                    _ => CursorStyle::Crosshair,
                };
                let owner = node.id.clone();
                div()
                    .id(ElementId::Name(
                        format!("{}-{}", node.id, declared.id).into(),
                    ))
                    .absolute()
                    .left(px(hx - size / 2.0))
                    .top(px(hy - size / 2.0))
                    .size(px(size))
                    .cursor(cursor)
                    .child(look)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            let hit = Hit::Handle {
                                owner: owner.clone(),
                                handle: declared.clone(),
                            };
                            this.press(event, hit, window, cx);
                        }),
                    )
                    .into_any_element()
            })
            .collect();

        // The box is the canvas's; everything painted inside it is the kind's.
        let element = div()
            .id(ElementId::Name(node.id.clone().into()))
            .absolute()
            .left(px(x))
            .top(px(y))
            .w(px(w))
            .flex()
            .flex_col()
            .map(|d| {
                if grows {
                    let least = kind::min_height(node).unwrap_or(mindmap::NODE_HEIGHT);
                    d.min_h(px(least as f32 * z))
                } else {
                    d.h(px(h))
                }
            })
            .when(draggable, |d| d.cursor_grab())
            .child(content)
            .when(grows, |d| {
                d.child(
                    painter(
                        move |bounds, window, _| {
                            let now = (bounds.size.height.as_f32() / z).round() as i64;
                            if now != height {
                                measured.borrow_mut().insert(measured_id, now);
                                window.refresh();
                            }
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
                )
            })
            .children(wash)
            .children(ring)
            .children(handles)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.press(event, Hit::Node(id.clone()), window, cx);
                }),
            );
        Some(element.into_any_element())
    }

    /// Edge labels, at the middle of their curves, and the one being typed.
    pub(super) fn labels(
        &self,
        theme: &Theme,
        shown: &Positions,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let (z, pan) = (self.editor.zoom(), self.editor.pan());
        let canvas = self.editor.painted();
        let nodes = canvas.lookup();
        canvas
            .edges
            .iter()
            .filter_map(|edge| {
                let editing = self.editing.as_ref().filter(|s| s.edge && s.id == edge.id);
                let text = self
                    .editor
                    .edge_kinds()
                    .get(edge)
                    .rules
                    .edit
                    .as_ref()
                    .map(|field| (field.read)(edge))
                    .filter(|label| !label.is_empty());
                if editing.is_none() && (text.is_none() || z < self.style.far_zoom) {
                    return None;
                }
                let middle = self.path_of(&nodes, shown, edge)?.middle();
                let (x, y) = (pan.x + middle.x * z, pan.y + middle.y * z);
                let body = match editing {
                    Some(session) => div()
                        .min_w(px(self.style.label.0 / 2.0 * z))
                        .child(session.input.clone())
                        .into_any_element(),
                    None => kind::text_style(div(), TextStyle::Callout, z)
                        .text_color(theme.text_muted)
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
                    .border_color(if picked { theme.accent } else { theme.border })
                    .bg(theme.surface_card)
                    .child(body)
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
                        .left(px(x - self.style.label.0 * z / 2.0))
                        .top(px(y - self.style.label.1 * z / 2.0))
                        .w(px(self.style.label.0 * z))
                        .h(px(self.style.label.1 * z))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(label)
                        .into_any_element(),
                )
            })
            .collect()
    }

    pub(super) fn edge_layer(
        &self,
        theme: &Theme,
        shown: &Positions,
        view: WeakEntity<Self>,
    ) -> impl IntoElement + use<> {
        let (z, pan) = (self.editor.zoom(), self.editor.pan());
        let grabbing = self.holding.is_some();
        let holding = self.held().is_some();
        let pending = &self.editor.pending;
        // Connectors a drop would cut, and the ones it would make.
        let cut: HashSet<&str> = pending
            .iter()
            .flat_map(|change| match change {
                Change::RemoveEdges { ids } => ids.as_slice(),
                _ => &[],
            })
            .map(String::as_str)
            .collect();
        // Each curve in view, its colour and its weight.
        let canvas = self.editor.painted();
        let nodes = canvas.lookup();
        let seen = self.editor.visible();
        let picked = self.editor.selected_edge();
        let mut strokes: Vec<(Path, Hsla, f32)> = canvas
            .edges
            .iter()
            .filter_map(|edge| {
                let path = self.path_of(&nodes, shown, edge)?;
                let (x0, y0, x1, y1) = path.hull();
                if seen.is_some_and(|(x, y, w, h)| x1 < x || x0 > x + w || y1 < y || y0 > y + h) {
                    return None;
                }
                let weight = self.editor.edge_kinds().get(edge).weight;
                if picked == Some(edge.id.as_str()) {
                    return Some((path, theme.accent, weight * 2.0));
                }
                let mut paint = edge
                    .color
                    .as_deref()
                    .and_then(|c| color(theme, c))
                    .unwrap_or(theme.border_strong);
                if cut.contains(edge.id.as_str()) {
                    paint = paint.opacity(self.style.cut);
                }
                Some((path, paint, weight))
            })
            .collect();
        strokes.extend(pending.iter().filter_map(|change| {
            let Change::AddEdge { edge, .. } = change else {
                return None;
            };
            Some((self.path_of(&nodes, shown, edge)?, theme.accent, 1.0))
        }));
        // The connector being drawn, out to the pointer.
        if let Some(Sketch::Connector { from, side, to }) = self.sketch()
            && let Some(node) = canvas.node(&from)
        {
            let at = shown
                .get(&from)
                .copied()
                .unwrap_or((node.x as f32, node.y as f32));
            let (start, out) = Rect::of(node, at).anchor(side);
            let reach = (to.x - start.x).abs().max((to.y - start.y).abs()) / 2.0;
            let c0 = point(start.x + out.x * reach, start.y + out.y * reach);
            let c1 = point(to.x - out.x * reach, to.y - out.y * reach);
            let mid = point((c0.x + c1.x) / 2.0, (c0.y + c1.y) / 2.0);
            let loose = Path {
                segments: vec![(start, c0, mid), (mid, c1, to)],
                from_out: out,
                to_out: point(-out.x, -out.y),
                from_arrow: false,
                to_arrow: true,
            };
            strokes.push((loose, theme.accent, 1.0));
        }
        let step = self.editor.snap().grid;
        let (arrow_size, style) = (self.style.arrow, self.style);
        let (grid, guides) = (self.overlays.grid.clone(), self.overlays.guides.clone());
        let caught = self.editor.guides.clone();
        let viewport = self.viewport.clone();
        painter(
            move |bounds, window, _| {
                if viewport.replace(Some(bounds)).map(|b| b.size) != Some(bounds.size) {
                    window.refresh();
                }
            },
            move |bounds, _, window, cx| {
                let origin = point(bounds.origin.x.as_f32(), bounds.origin.y.as_f32());
                let screen =
                    |p: Point<f32>| point(origin.x + pan.x + p.x * z, origin.y + pan.y + p.y * z);
                let frame = Frame {
                    origin,
                    pan,
                    zoom: z,
                    width: bounds.size.width.as_f32(),
                    height: bounds.size.height.as_f32(),
                    style,
                };
                if let Some(step) = step {
                    grid(&frame, step, window, cx);
                }
                for (path, paint, weight) in strokes {
                    let mut stroke = PathBuilder::stroke(px(weight * z.max(1.0)));
                    for (ix, (a, c, b)) in path.segments.iter().enumerate() {
                        if ix == 0 {
                            stroke.move_to(pt(screen(*a)));
                        }
                        stroke.curve_to(pt(screen(*b)), pt(screen(*c)));
                    }
                    if let Ok(stroke) = stroke.build() {
                        window.paint_path(stroke, paint);
                    }
                    if path.to_arrow {
                        arrow(
                            window,
                            screen(path.end()),
                            path.to_out,
                            arrow_size * z,
                            paint,
                        );
                    }
                    if path.from_arrow {
                        arrow(
                            window,
                            screen(path.start()),
                            path.from_out,
                            arrow_size * z,
                            paint,
                        );
                    }
                }
                guides(&frame, &caught, window, cx);
                // Window-wide, so the hand stays closed wherever the pointer
                // carries the node.
                if holding {
                    window.set_window_cursor_style(CursorStyle::ClosedHand);
                }
                // A held press follows the pointer past the view's edge, so it
                // listens to the window rather than to this element.
                if grabbing {
                    let moves = view.clone();
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                        if phase == DispatchPhase::Bubble {
                            let _ = moves.update(cx, |this, cx| this.moved(event, window, cx));
                        }
                    });
                    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                        let held = matches!(event.button, MouseButton::Left | MouseButton::Middle);
                        if phase == DispatchPhase::Bubble && held {
                            let _ =
                                view.update(cx, |this, cx| this.lift(event.position, window, cx));
                        }
                    });
                }
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    }
}

/// The boxes an edge joins, where they paint. An end that names a handle
/// leaves the side that handle sits on; one that names none leaves the side
/// the spec gave it, else the side the boxes face each other on.
pub(super) fn ends_of(
    nodes: &HashMap<&str, &Node>,
    shown: &Positions,
    edge: &Edge,
    kinds: &Kinds,
) -> Option<Ends> {
    let rect = |id: &str| {
        let node = *nodes.get(id)?;
        let at = shown
            .get(id)
            .copied()
            .unwrap_or((node.x as f32, node.y as f32));
        Some(Rect::of(node, at))
    };
    // Where a named handle sits, offset and all — more than a side can say.
    let anchor = |id: &str, which: Which| {
        let named = edge::handle_of(edge, which)?;
        let node = *nodes.get(id)?;
        (kinds.get(&node.kind).rules.handles)(node)
            .into_iter()
            .find(|declared| declared.id == named)?
            .spot
            .on(rect(id)?)
    };
    Some(Ends {
        from: rect(&edge.from_node)?,
        to: rect(&edge.to_node)?,
        from_anchor: anchor(&edge.from_node, Which::From),
        to_anchor: anchor(&edge.to_node, Which::To),
        from_side: edge.from_side,
        to_side: edge.to_side,
        from_end: edge.from_end.unwrap_or(End::None),
        to_end: edge.to_end.unwrap_or(End::Arrow),
    })
}

pub(super) fn arrow(window: &mut Window, tip: Point<f32>, out: Point<f32>, size: f32, color: Hsla) {
    let base = point(tip.x + out.x * size, tip.y + out.y * size);
    let half = point(-out.y * size / 2.0, out.x * size / 2.0);
    let mut path = PathBuilder::fill();
    path.move_to(pt(tip));
    path.line_to(pt(point(base.x + half.x, base.y + half.y)));
    path.line_to(pt(point(base.x - half.x, base.y - half.y)));
    path.close();
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

pub(super) fn pt(p: Point<f32>) -> Point<Pixels> {
    point(px(p.x), px(p.y))
}
