//! Presses, drags, edge handles, wheel, pinch and drift.

use super::*;

impl CanvasView {
    /// What a press landed on. A node and its handles answer for themselves,
    /// as they are painted over the canvas; the background asks the edges.
    pub(super) fn hit_at(&self, position: Point<Pixels>) -> Hit {
        match self.edge_at(position) {
            Some(id) => Hit::Edge(id),
            None => Hit::Nothing,
        }
    }

    pub(super) fn pointer(&self, screen: Point<Pixels>, hit: Hit) -> Pointer {
        Pointer {
            screen,
            at: self.canvas_point(screen),
            button: MouseButton::Left,
            modifiers: Modifiers::default(),
            clicks: 1,
            hit,
        }
    }

    /// Offer a press to the tools: the first that takes it holds the pointer
    /// until it comes up.
    pub(super) fn press(
        &mut self,
        event: &MouseDownEvent,
        hit: Hit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // A press inside what is being typed in belongs to the editor.
        let typing = self.editing.as_ref().is_some_and(|session| match &hit {
            Hit::Node(id) => !session.edge && session.id == *id,
            Hit::Edge(id) => session.edge && session.id == *id,
            _ => false,
        });
        if typing {
            return;
        }
        self.stop_editing(window, cx);
        window.focus(&self.focus, cx);
        let pointer = Pointer {
            button: event.button,
            modifiers: event.modifiers,
            clicks: event.click_count,
            ..self.pointer(event.position, hit)
        };
        self.aim = Some(event.position);
        let mut wish = Wish::Nothing;
        self.holding = None;
        for (ix, tool) in self.tools.iter_mut().enumerate() {
            let mut hand = Hand::new(&mut self.editor);
            if tool.press(&pointer, &mut hand) {
                wish = hand.wish;
                self.holding = Some(ix);
                break;
            }
        }
        self.announce(cx);
        cx.notify();
        self.grant(wish, window, cx);
    }

    /// What the tool asked of the view once its commands had landed.
    pub(super) fn grant(&mut self, wish: Wish, window: &mut Window, cx: &mut Context<Self>) {
        match wish {
            Wish::Nothing => {}
            Wish::Enter(id) => {
                let node = self.editor.canvas().node(&id).cloned();
                let open = node
                    .as_ref()
                    .and_then(|node| self.editor.kinds().get(&node.kind).open.clone());
                match (open, node) {
                    (Some(open), Some(node)) => open(&node, cx),
                    _ => self.edit(id, None, window, cx),
                }
            }
            Wish::EditEdge(id) => self.edit_edge(id, window, cx),
            // Typing into what was added joins the add in one undo step.
            Wish::EditAdded(id) => {
                let group = self.editor.last_group();
                self.edit(id, group, window, cx);
            }
        }
    }

    /// Hand the held tool where the pointer is now.
    pub(super) fn feed(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(ix) = self.holding else {
            return;
        };
        let pointer = self.pointer(position, Hit::Nothing);
        let mut hand = Hand::new(&mut self.editor);
        self.tools[ix].drag(&pointer, &mut hand);
        self.announce(cx);
        cx.notify();
    }

    /// The button came up: the held tool lands its gesture.
    pub(super) fn lift(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        (self.aim, self.drifted) = (None, None);
        let Some(ix) = self.holding.take() else {
            return;
        };
        let pointer = self.pointer(position, Hit::Nothing);
        let mut hand = Hand::new(&mut self.editor);
        self.tools[ix].release(&pointer, &mut hand);
        let wish = hand.wish;
        self.announce(cx);
        cx.notify();
        self.grant(wish, window, cx);
    }

    /// Give up the gesture in hand, as `escape` does.
    pub(super) fn cancel(&mut self, cx: &mut Context<Self>) {
        let Some(ix) = self.holding.take() else {
            return;
        };
        let mut hand = Hand::new(&mut self.editor);
        self.tools[ix].cancel(&mut hand);
        self.announce(cx);
        cx.notify();
    }

    /// The node a connector being drawn would reach.
    pub(super) fn connect_target(&self) -> Option<String> {
        let Some(Sketch::Connector { from, to, .. }) = self.sketch() else {
            return None;
        };
        let at = (to.x.round() as i64, to.y.round() as i64);
        self.editor.node_under(at, &from).map(str::to_owned)
    }

    /// Where an edge runs, as its kind draws it.
    pub(super) fn path_of(
        &self,
        nodes: &HashMap<&str, &Node>,
        shown: &Positions,
        edge: &Edge,
    ) -> Option<Path> {
        let ends = ends_of(nodes, shown, edge, self.editor.kinds())?;
        Some((self.editor.edge_kinds().get(edge).path)(&ends))
    }

    /// The handles the picked edge's kind declares, along its path.
    pub(super) fn edge_handles(
        &self,
        theme: &Theme,
        shown: &Positions,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(id) = self.editor.selected_edge().map(str::to_owned) else {
            return Vec::new();
        };
        if self.holding.is_some() || !self.editor.edge_can(&id, Capability::Reconnectable) {
            return Vec::new();
        }
        let canvas = self.editor.painted();
        let nodes = canvas.lookup();
        let Some(edge) = canvas.edge(&id) else {
            return Vec::new();
        };
        let Some(path) = self.path_of(&nodes, shown, edge) else {
            return Vec::new();
        };
        let (z, pan) = (self.editor.zoom(), self.editor.pan());
        (self.editor.edge_kinds().get(edge).rules.handles)(edge)
            .into_iter()
            .filter(|declared| matches!(declared.role, Role::Reconnect(_)))
            .filter_map(|declared| {
                let at = declared.spot.along(&path)?;
                let (x, y) = (pan.x + at.x * z, pan.y + at.y * z);
                let owner = id.clone();
                Some(
                    div()
                        .id(ElementId::Name(
                            format!("edge-handle-{}-{}", id, declared.id).into(),
                        ))
                        .absolute()
                        .left(px(x - self.style.handle / 2.0))
                        .top(px(y - self.style.handle / 2.0))
                        .size(px(self.style.handle))
                        .rounded_full()
                        .border_1()
                        .border_color(theme.accent)
                        .bg(theme.surface_card)
                        .cursor(CursorStyle::Crosshair)
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
            .collect()
    }

    /// The topmost edge passing within its kind's reach of a window position.
    pub(super) fn edge_at(&self, position: Point<Pixels>) -> Option<String> {
        let at = self.canvas_point(position);
        let zoom = self.editor.zoom();
        let canvas = self.editor.painted();
        let nodes = canvas.lookup();
        canvas
            .edges
            .iter()
            .rev()
            .find(|edge| {
                let reach = self.editor.edge_kinds().get(edge).reach / zoom;
                self.path_of(&nodes, &self.shown, edge)
                    .is_some_and(|path| path.distance(at) <= reach)
            })
            .map(|edge| edge.id.clone())
    }

    pub(super) fn wheel(
        &mut self,
        event: &ScrollWheelEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let delta = event.delta.pixel_delta(window.line_height());
        if event.modifiers.platform || event.modifiers.control {
            let zoom =
                self.editor.zoom() * (delta.y.as_f32() * self.editor.options().wheel_zoom).exp();
            let anchor = self.local(event.position);
            self.update_editor(cx, |editor| editor.zoom_about(zoom, anchor));
        } else {
            self.update_editor(cx, |editor| {
                editor.pan_by(delta.x.as_f32(), delta.y.as_f32())
            });
        }
        cx.stop_propagation();
    }

    /// The pointer moved with a button down: the tool holding it hears.
    pub(super) fn moved(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !event.dragging() {
            self.lift(event.position, window, cx);
            return;
        }
        self.aim = Some(event.position);
        self.feed(event.position, cx);
    }

    pub(super) fn pinch(&mut self, event: &PinchEvent, _: &mut Window, cx: &mut Context<Self>) {
        let zoom = self.editor.zoom() * (1.0 + event.delta);
        let anchor = self.local(event.position);
        self.update_editor(cx, |editor| editor.zoom_about(zoom, anchor));
    }

    /// Pan while a held press rests near the view's edge, carrying what it
    /// holds along.
    pub(super) fn drift(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let drifting = self.holding.is_some_and(|ix| self.tools[ix].drifts());
        let (Some(aim), Some(bounds), true) = (self.aim, self.viewport.get(), drifting) else {
            self.drifted = None;
            return;
        };
        let velocity = point(
            ui::scroll::drift_velocity(aim.x, bounds.left(), bounds.right()),
            ui::scroll::drift_velocity(aim.y, bounds.top(), bounds.bottom()),
        );
        if velocity.x == 0.0 && velocity.y == 0.0 {
            self.drifted = None;
            return;
        }
        window.request_animation_frame();
        let now = Instant::now();
        // The first frame starts the clock; there is no interval to travel yet.
        let Some(last) = self.drifted.replace(now) else {
            return;
        };
        let step = (now - last)
            .as_secs_f32()
            .min(self.editor.options().drift_step);
        self.editor.pan_by(velocity.x * step, velocity.y * step);
        // The pointer has not moved, but the canvas under it has.
        self.feed(aim, cx);
    }
}
