//! Node positions, node and label painting, and the edge layer.

use crate::view::{body, *};

impl CanvasView {
    /// Where each node paints this frame. A node the document moved glides
    /// there from where it was painted. Active gestures and newly painted
    /// nodes use their preview positions directly.
    pub(super) fn positions(&mut self, changed: bool, reduced: bool, window: &mut Window) -> bool {
        let direct = self.holding.is_some();
        let finish = self.glide.is_some() && (direct || reduced);
        if direct || reduced {
            self.glide = None;
        } else if changed {
            let moved = self
                .geometry
                .positions
                .iter()
                .any(|(id, at)| self.shown.get(id).is_some_and(|shown| shown != at));
            if moved {
                self.glide = Some(Glide {
                    from: self.shown.clone(),
                    to: self.geometry.positions.clone(),
                    since: Instant::now(),
                });
            } else {
                self.glide = None;
            }
        }
        let Some(glide) = &self.glide else {
            if changed || finish {
                self.shown.clone_from(&self.geometry.positions);
            }
            return finish;
        };
        let total = LAYOUT.total().mul_f32(motion::speed_scale());
        let raw = glide.since.elapsed().as_secs_f32() / total.as_secs_f32().max(f32::EPSILON);
        let t = LAYOUT.progress(raw);
        self.shown.retain(|id, _| glide.to.contains_key(id));
        for (id, &at) in &glide.to {
            let at = glide.from.get(id).map_or(at, |&from| {
                (motion::lerp(from.0, at.0, t), motion::lerp(from.1, at.1, t))
            });
            if let Some(shown) = self.shown.get_mut(id) {
                *shown = at;
            } else {
                self.shown.insert(id.clone(), at);
            }
        }
        if raw >= 1.0 {
            self.glide = None;
        } else {
            window.request_animation_frame();
        }
        true
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
        let editing = self
            .editing
            .as_ref()
            .filter(|s| s.node_id() == Some(node.id.as_str()));
        // A node being typed in keeps the editor's own cursor.
        let draggable = editing.is_none();
        let selected = picked.contains(&node.id.as_str());
        let mark = Mark {
            width: w,
            height: h,
            zoom: z,
            style: self.style,
        };
        let heights = self.content_heights.clone();
        let (measured_node, width) = (node.id.clone(), node.width);
        let look = Look {
            card: Card {
                zoom: z,
                size: size(px(w), px(h)),
                typography: presentation::text_typography(cx, node.text_size(), z),
                selected,
                sizing: self.editor.kinds().sizing(node),
                measure: Measure {
                    zoom: z,
                    report: Rc::new(move |height| {
                        heights
                            .borrow_mut()
                            .insert(measured_node.clone(), (width, height));
                    }),
                },
            },
            editor: editing.and_then(|session| session.input.inline()),
        };
        let controls = kind
            .controls
            .as_ref()
            .filter(|_| editing.is_none() && !self.exporting)
            .and_then(|controls| controls(node, &look.card, window, cx));
        let content = if kind.retained
            && editing.is_none()
            && !self.exporting
            && look.card.sizing == Sizing::Fixed
        {
            body::render(node, look.card, kind.render.clone(), window, cx)
        } else {
            (kind.render)(node, look, window, cx)
        };
        let grows = self.editor.kinds().sizing(node) == Sizing::Grows;
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
        let owner = cx.entity_id();
        let wash = target.then(|| (self.overlays.drop)(&mark, window, cx));
        let ring = (selected || target).then(|| (self.overlays.ring)(&mark, window, cx));
        let handles: Vec<AnyElement> = declared
            .into_iter()
            .filter_map(|declared| {
                let size = self.style.handle;
                let bounds = declared.bounds(Rect { x: 0., y: 0., w, h }, size)?;
                let look = (self.overlays.handle)(
                    &Mark {
                        width: size,
                        height: size,
                        ..mark
                    },
                    window,
                    cx,
                );
                let axes = declared.spot.resize_axes();
                let cursor = declared.cursor();
                let owner = node.id.clone();
                let border_handle =
                    declared.role == Role::Resize && axes.is_some_and(|(x, y)| x == 0 || y == 0);
                Some(
                    div()
                        .id(ElementId::Name(
                            format!("{}-{}", node.id, declared.id).into(),
                        ))
                        .absolute()
                        .left(px(bounds.origin.x))
                        .top(px(bounds.origin.y))
                        .w(px(bounds.size.width))
                        .h(px(bounds.size.height))
                        .cursor(cursor)
                        .when(!border_handle, |element| element.child(look))
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
                        .into_any_element(),
                )
            })
            .collect();

        let surface = draggable.then(|| {
            let id = node.id.clone();
            div()
                .id(ElementId::Name(
                    format!("canvas-surface-{}", node.id).into(),
                ))
                .absolute()
                .inset_0()
                .block_mouse_except_scroll()
                .cursor_grab()
                .on_any_mouse_down(
                    cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                        if event.button == MouseButton::Right {
                            this.request_context_menu(event, window, cx);
                        } else {
                            cx.stop_propagation();
                            this.press(event, Hit::Node(id.clone()), window, cx);
                        }
                    }),
                )
        });

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
                    let least = if node.is_label() {
                        1
                    } else {
                        kind::min_height(node).unwrap_or(mindmap::NODE_HEIGHT)
                    };
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
                        move |bounds, _, cx| {
                            let now = (bounds.size.height.as_f32() / z).round() as i64;
                            if now != height {
                                measured.borrow_mut().insert(measured_id, now);
                                cx.notify(owner);
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
            .children(surface)
            .children(wash)
            .children(ring)
            .children(controls)
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

    pub(super) fn edge_layer(
        &self,
        theme: &Theme,
        view: WeakEntity<Self>,
    ) -> impl IntoElement + use<> {
        let (z, pan) = (self.editor.zoom(), self.editor.pan());
        let grabbing = self.holding.is_some();
        let cursor = self.holding.and_then(|ix| self.tools[ix].cursor());
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
        let seen = self.editor.visible();
        let picked = self.editor.selected_edge();
        let mut strokes: Vec<(Rc<Path>, Stroke, Hsla, f32)> = canvas
            .edges
            .iter()
            .filter_map(|edge| {
                let path = self.geometry.paths.get(&edge.id)?.clone();
                let (x0, y0, x1, y1) = path.hull();
                if seen.is_some_and(|(x, y, w, h)| x1 < x || x0 > x + w || y1 < y || y0 > y + h) {
                    return None;
                }
                let weight = self.editor.edge_kinds().get(edge).weight;
                let stroke = edge.style.unwrap_or_default();
                if picked == Some(edge.id.as_str()) {
                    return Some((path, stroke, theme.accent, weight * 2.0));
                }
                let mut paint = edge
                    .color
                    .as_deref()
                    .and_then(|c| color(theme, c))
                    .unwrap_or(theme.border_strong);
                if cut.contains(edge.id.as_str()) {
                    paint = paint.opacity(self.style.cut);
                }
                Some((path, stroke, paint, weight))
            })
            .collect();
        strokes.extend(pending.iter().filter_map(|change| {
            let Change::AddEdge { edge, .. } = change else {
                return None;
            };
            let path = self.geometry.paths.get(&edge.id)?.clone();
            Some((path, Stroke::Solid, theme.accent, 1.0))
        }));
        // The connector being drawn, out to the pointer.
        if let Some(Sketch::Connector { from, side, to }) = self.sketch()
            && let Some(node) = canvas.node(&from)
        {
            let at = self
                .shown
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
                from_end: End::None,
                to_end: End::Arrow,
            };
            strokes.push((Rc::new(loose), Stroke::Solid, theme.accent, 1.0));
        }
        let step = self.editor.snap().grid;
        let (arrow_size, style) = (self.style.arrow, self.style);
        let (grid, guides) = (self.overlays.grid.clone(), self.overlays.guides.clone());
        let caught = self.editor.guides.clone();
        let viewport = self.viewport.clone();
        let owner = view.entity_id();
        painter(
            move |bounds, window, cx| {
                if viewport.replace(Some(bounds)).map(|b| b.size) != Some(bounds.size) {
                    cx.notify(owner);
                }
                window.insert_hitbox(bounds, HitboxBehavior::Normal)
            },
            move |bounds, hitbox, window, cx| {
                let zoom_view = view.clone();
                let hitbox = hitbox.clone();
                window.on_mouse_event(move |event: &ScrollWheelEvent, phase, window, cx| {
                    if phase == DispatchPhase::Capture
                        && hitbox.should_handle_scroll(window)
                        && (event.modifiers.platform || event.modifiers.control)
                    {
                        let _ = zoom_view.update(cx, |this, cx| this.wheel(event, window, cx));
                    }
                });
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
                for (path, stroke, paint, weight) in strokes {
                    crate::paint::edge(
                        window,
                        &path,
                        screen,
                        stroke,
                        weight * z.max(1.0),
                        arrow_size * z,
                        paint,
                    );
                }
                guides(&frame, &caught, window, cx);
                if let Some(cursor) = cursor {
                    window.set_window_cursor_style(cursor);
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
                            let _ = view.update(cx, |this, cx| {
                                this.modifiers = event.modifiers;
                                this.lift(event.position, window, cx)
                            });
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
