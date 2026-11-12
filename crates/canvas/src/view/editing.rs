use crate::{
    change::Change,
    edge::EdgeField,
    edit::CanvasEvent,
    kind::{Control, Field},
    model::{self, Node},
    presentation,
    view::{CanvasView, keys},
};
use editor::{Chrome, Editor, EditorEvent};
use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, Focusable, MouseButton, Point, Subscription,
    Window, div, point, prelude::*, px,
};
use theme::{Metrics, TextStyle, Theme};
use ui::{
    input::{FieldEvent, Shape, TextField},
    popover,
    widgets::Button,
};
use url::{ParseError, Url};

pub(super) const FIELD_CONTEXT: &str = "BezelCanvasField";
pub(super) const TEXT_CONTEXT: &str = "BezelCanvasText";

pub(super) enum Target {
    Node(String),
    Edge(String),
    New { node: Box<Node>, at: (i64, i64) },
}

pub(super) enum Input {
    Document(Entity<Editor>),
    Field {
        field: Entity<TextField>,
        control: Control,
    },
}

impl Input {
    fn source(&self, cx: &App) -> String {
        match self {
            Self::Document(input) => input.read(cx).source(),
            Self::Field { field, .. } => field.read(cx).content().to_string(),
        }
    }

    pub(super) fn inline(&self) -> Option<AnyElement> {
        match self {
            Self::Document(input) => Some(input.clone().into_any_element()),
            Self::Field { field, control } => {
                (*control != Control::Url).then(|| field.clone().into_any_element())
            }
        }
    }

    fn focus(&self, cx: &App) -> FocusHandle {
        match self {
            Self::Document(input) => input.focus_handle(cx),
            Self::Field { field, .. } => field.focus_handle(cx),
        }
    }
}

pub(super) struct Session {
    pub target: Target,
    pub input: Input,
    added: bool,
    error: Option<String>,
    _changes: Subscription,
}

impl Session {
    pub fn node_id(&self) -> Option<&str> {
        match &self.target {
            Target::Node(id) => Some(id),
            _ => None,
        }
    }

    pub fn edge_id(&self) -> Option<&str> {
        match &self.target {
            Target::Edge(id) => Some(id),
            _ => None,
        }
    }
}

impl CanvasView {
    pub fn create_node(
        &mut self,
        node: Node,
        at: (i64, i64),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sync_document(cx);
        if self.editor.options().read_only {
            return;
        }
        self.stop_editing(window, cx);
        let field = self.editor.kinds().get(&node.kind).rules.edit.as_ref();
        if field.is_some_and(|field| (field.control)(&node) == Control::Url) {
            let value = field.map(|field| (field.read)(&node)).unwrap_or_default();
            self.start_editing(
                Target::New {
                    node: Box::new(node),
                    at,
                },
                Control::Url,
                &value,
                None,
                window,
                cx,
            );
        } else {
            let added = self.update_editor(cx, |editor| editor.add_node(node, at));
            self.edit_added(added, window, cx);
        }
    }

    pub(super) fn edit(
        &mut self,
        id: String,
        group: Option<u64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.editor.options().read_only {
            return;
        }
        self.stop_editing(window, cx);
        let Some(node) = self.editor.canvas().node(&id) else {
            return;
        };
        let Some(field) = self.editor.kinds().get(&node.kind).rules.edit.as_ref() else {
            return;
        };
        let control = (field.control)(node);
        if control == Control::External {
            cx.emit(CanvasEvent::EditRequested { id });
            return;
        }
        let value = (field.read)(node);
        self.start_editing(Target::Node(id), control, &value, group, window, cx);
    }

    pub(super) fn edit_edge(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.editor.options().read_only {
            return;
        }
        self.stop_editing(window, cx);
        let Some(edge) = self.editor.canvas().edge(&id) else {
            return;
        };
        let Some(field) = self.editor.edge_kinds().get(edge).rules.edit.as_ref() else {
            return;
        };
        let value = (field.read)(edge);
        self.start_editing(Target::Edge(id), Control::Name, &value, None, window, cx);
    }

    pub fn edit_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_document(cx);
        if let Some(id) = self.editor.selected_edge().map(str::to_owned) {
            self.edit_edge(id, window, cx);
        } else if let Some(id) = self.editor.selected().map(str::to_owned) {
            self.edit(id, None, window, cx);
        }
    }

    fn start_editing(
        &mut self,
        target: Target,
        control: Control,
        value: &str,
        group: Option<u64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (input, changes) = if control == Control::Document {
            let input = cx.new(|cx| {
                Editor::new(value, cx)
                    .with_placeholder("Type text…")
                    .with_chrome(Chrome {
                        handle: false,
                        slash: false,
                        language: false,
                        paste: false,
                        mention: false,
                    })
            });
            let changes = cx.subscribe(&input, |this, _, event: &EditorEvent, cx| {
                if *event == EditorEvent::Changed {
                    this.editing_changed(cx);
                }
            });
            (Input::Document(input), changes)
        } else {
            let field = cx.new(|cx| {
                let mut field = TextField::new(cx)
                    .with_frame(control == Control::Url)
                    .with_placeholder(match control {
                        Control::Url => "Paste a URL…",
                        Control::Name => "Name…",
                        _ => "Type text…",
                    })
                    .with_key_context(if control == Control::Text {
                        TEXT_CONTEXT
                    } else {
                        FIELD_CONTEXT
                    });
                if control == Control::Text {
                    field = field.with_shape(Shape::Grow {
                        min: 1,
                        max: usize::MAX,
                    });
                }
                field.set_content(value.to_owned(), cx);
                if control == Control::Name {
                    field.select(0..value.len(), cx);
                }
                field
            });
            let changes = cx.subscribe(&field, |this, _, event: &FieldEvent, cx| {
                if matches!(event, FieldEvent::Changed(_)) {
                    this.editing_changed(cx);
                }
            });
            (Input::Field { field, control }, changes)
        };
        window.focus(&input.focus(cx), cx);
        window.prevent_default();
        self.editor.group = Some(group.unwrap_or_else(|| self.editor.next_group()));
        self.editing = Some(Session {
            target,
            input,
            added: group.is_some(),
            error: None,
            _changes: changes,
        });
        self.sync_input(cx);
        cx.notify();
    }

    fn editing_changed(&mut self, cx: &mut Context<Self>) {
        let Some(session) = &mut self.editing else {
            return;
        };
        if matches!(
            session.input,
            Input::Field {
                control: Control::Url,
                ..
            }
        ) {
            session.error = None;
            cx.notify();
            return;
        }
        let value = session.input.source(cx);
        let change = match &session.target {
            Target::Node(id) => {
                let Some(mut node) = self.editor.canvas().node(id).cloned() else {
                    return;
                };
                let Some(Field { write, .. }) = &self.editor.kinds().get(&node.kind).rules.edit
                else {
                    return;
                };
                write(&mut node, value);
                Change::UpdateNode { node }
            }
            Target::Edge(id) => {
                let Some(mut edge) = self.editor.canvas().edge(id).cloned() else {
                    return;
                };
                let Some(EdgeField { write, .. }) = &self.editor.edge_kinds().get(&edge).rules.edit
                else {
                    return;
                };
                write(&mut edge, value);
                Change::UpdateEdge { edge }
            }
            Target::New { .. } => return,
        };
        self.update_editor(cx, |editor| editor.submit([change]));
    }

    pub(super) fn sync_input(&self, cx: &mut Context<Self>) {
        let Some(session) = &self.editing else { return };
        let node = session
            .node_id()
            .and_then(|id| self.editor.canvas().node(id));
        let edge = session
            .edge_id()
            .and_then(|id| self.editor.canvas().edge(id));
        let size = node
            .map(Node::text_size)
            .or_else(|| edge.map(|edge| edge.text_size()))
            .unwrap_or(model::TEXT_SIZE);
        let mut typography = presentation::text_typography(cx, size, self.editor.zoom());
        if node.is_some_and(|node| node.kind == model::GROUP) {
            typography.body = Metrics::from(TextStyle::Callout).scaled(self.editor.zoom());
        }
        let color = node.and_then(|node| node.text_color(Theme::of(cx)));
        match &session.input {
            Input::Document(input) => input.update(cx, |input, cx| {
                if input.typography != Some(typography) || input.text_color != color {
                    input.typography = Some(typography);
                    input.text_color = color;
                    cx.notify();
                }
            }),
            Input::Field { field, control } if *control != Control::Url => {
                field.update(cx, |field, cx| {
                    field.set_metrics(typography.body, cx);
                    if field.text_color != color {
                        field.text_color = color;
                        cx.notify();
                    }
                })
            }
            _ => {}
        }
    }

    pub fn stop_editing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let editing = self.editing.is_some();
        self.end_editing(cx);
        if editing {
            window.focus(&self.focus, cx);
        }
    }

    pub(super) fn end_editing(&mut self, cx: &mut Context<Self>) {
        if let Some(session) = self.editing.take() {
            if session.added
                && session.input.source(cx).trim().is_empty()
                && let Some(id) = session.node_id()
                && self
                    .editor
                    .canvas()
                    .node(id)
                    .is_some_and(|node| node.kind == model::TEXT)
            {
                self.update_editor(cx, |editor| editor.cancel_group());
            }
            cx.notify();
        }
        self.editor.group = None;
    }

    pub fn confirm_editing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_document(cx);
        if self.editor.options().read_only {
            return self.stop_editing(window, cx);
        }
        let Some(session) = &self.editing else { return };
        if !matches!(
            session.input,
            Input::Field {
                control: Control::Url,
                ..
            }
        ) {
            return self.stop_editing(window, cx);
        }
        let value = session.input.source(cx);
        let value = value.trim();
        if value.is_empty()
            || !matches!(
                Url::parse(value),
                Ok(_) | Err(ParseError::RelativeUrlWithoutBase)
            )
        {
            self.editing.as_mut().unwrap().error = Some("Enter a valid URL or path.".into());
            cx.notify();
            return;
        }
        let session = self.editing.take().unwrap();
        match session.target {
            Target::New { mut node, at } => {
                if let Some(field) = &self.editor.kinds().get(&node.kind).rules.edit {
                    (field.write)(&mut node, value.to_owned());
                }
                self.update_editor(cx, |editor| editor.add_node(*node, at));
            }
            Target::Node(id) => {
                if let Some(mut node) = self.editor.canvas().node(&id).cloned() {
                    if let Some(field) = &self.editor.kinds().get(&node.kind).rules.edit {
                        (field.write)(&mut node, value.to_owned());
                    }
                    self.update_editor(cx, |editor| editor.submit([Change::UpdateNode { node }]));
                }
            }
            Target::Edge(_) => {}
        }
        self.editor.group = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    pub(super) fn edit_prompt(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let session = self.editing.as_ref()?;
        let Input::Field {
            field,
            control: Control::Url,
        } = &session.input
        else {
            return None;
        };
        let at = match &session.target {
            Target::New { at, .. } => *at,
            Target::Node(id) => {
                let node = self.editor.canvas().node(id)?;
                (node.x, node.y + node.height)
            }
            Target::Edge(_) => return None,
        };
        let origin = self
            .viewport
            .get()
            .map_or(Point::default(), |bounds| bounds.origin);
        let pan = self.editor.pan();
        let zoom = self.editor.zoom();
        let position = point(
            origin.x + px(pan.x + at.0 as f32 * zoom),
            origin.y + px(pan.y + at.1 as f32 * zoom),
        );
        let theme = Theme::of(cx).clone();
        let focus = field.focus_handle(cx);
        let content = popover::popover_card(&theme)
            .w(px(360.).min((window.viewport_size().width - px(32.)).max(px(0.))))
            .flex()
            .flex_col()
            .gap(px(8.))
            .p(px(12.))
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                window.focus(&focus, cx);
                cx.stop_propagation();
            })
            .on_mouse_down_out(cx.listener(|this, _, window, cx| this.stop_editing(window, cx)))
            .child("Link")
            .child(field.clone())
            .child(
                div().flex().justify_end().child(
                    Button::new(
                        "canvas-save-link",
                        if matches!(session.target, Target::New { .. }) {
                            "Add link"
                        } else {
                            "Save"
                        },
                    )
                    .on_press(cx.listener(|this, _, window, cx| this.confirm_editing(window, cx))),
                ),
            )
            .children(
                session
                    .error
                    .as_ref()
                    .map(|error| div().text_color(theme.danger_muted).child(error.clone())),
            )
            .on_action(cx.listener(|this, _: &keys::ConfirmEditing, window, cx| {
                this.confirm_editing(window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &keys::StopEditing, window, cx| {
                    this.stop_editing(window, cx)
                }),
            );
        Some(popover::menu_at(
            "canvas-link-input",
            position,
            content.into_any_element(),
            None,
        ))
    }
}

#[cfg(test)]
#[path = "../../tests/unit/editing.rs"]
mod tests;
