use crate::{
    change::{self, Change},
    model::Canvas,
};
use gpui::{Context, EntityId, EventEmitter};
use std::rc::Rc;

#[derive(Clone)]
struct Step {
    changes: Vec<Change>,
    group: Option<u64>,
}

#[derive(Clone, Default)]
struct History {
    undo: Vec<Step>,
    redo: Vec<Step>,
    groups: u64,
}

/// Content and undo history shared by canvas views. Selection and viewport
/// belong to each view.
#[derive(Clone)]
pub struct Document {
    pub(crate) canvas: Rc<Canvas>,
    history: Rc<History>,
    pub(crate) revision: u64,
}

pub struct DocumentEvent {
    pub(crate) by: Option<EntityId>,
    pub change: DocumentChange,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DocumentChange {
    Edited(Vec<Change>),
    Reloaded,
}

impl EventEmitter<DocumentEvent> for Document {}

impl std::ops::Deref for Document {
    type Target = Canvas;

    fn deref(&self) -> &Canvas {
        &self.canvas
    }
}

impl Document {
    pub fn new(canvas: Canvas) -> Self {
        Self {
            canvas: Rc::new(canvas),
            history: Rc::default(),
            revision: 0,
        }
    }

    /// Apply an undoable batch from outside a canvas view.
    pub fn edit(&mut self, changes: Vec<Change>, cx: &mut Context<Self>) {
        if changes.is_empty() {
            return;
        }
        self.apply(&changes, None, crate::Options::default().history);
        cx.emit(DocumentEvent {
            by: None,
            change: DocumentChange::Edited(changes),
        });
        cx.notify();
    }

    /// Load an externally saved document, clearing history only when it changed.
    pub fn reload(&mut self, canvas: Canvas, cx: &mut Context<Self>) {
        if self.canvas.as_ref() == &canvas {
            return;
        }
        let revision = self.revision + 1;
        *self = Self::new(canvas);
        self.revision = revision;
        cx.emit(DocumentEvent {
            by: None,
            change: DocumentChange::Reloaded,
        });
        cx.notify();
    }

    pub(crate) fn apply(&mut self, changes: &[Change], group: Option<u64>, limit: usize) {
        let undo = change::apply_all(Rc::make_mut(&mut self.canvas), changes);
        self.revision += 1;
        if undo.is_empty() {
            return;
        }
        let history = Rc::make_mut(&mut self.history);
        history.redo.clear();
        match history.undo.last_mut() {
            Some(step) if group.is_some() && step.group == group => {
                step.changes.splice(0..0, undo);
            }
            _ => {
                history.undo.push(Step {
                    changes: undo,
                    group,
                });
                if history.undo.len() > limit {
                    history.undo.remove(0);
                }
            }
        }
    }

    pub(crate) fn derive(&mut self, change: &Change) {
        change::apply(Rc::make_mut(&mut self.canvas), change);
        self.revision += 1;
    }

    pub(crate) fn cancel_group(&mut self, group: u64) -> Option<Vec<Change>> {
        let history = Rc::make_mut(&mut self.history);
        if history.undo.last()?.group != Some(group) {
            return None;
        }
        let step = history.undo.pop()?;
        change::apply_all(Rc::make_mut(&mut self.canvas), &step.changes);
        self.revision += 1;
        Some(step.changes)
    }

    pub(crate) fn undo(&mut self) -> Option<Vec<Change>> {
        let history = Rc::make_mut(&mut self.history);
        let step = history.undo.pop()?;
        let changes = change::apply_all(Rc::make_mut(&mut self.canvas), &step.changes);
        history.redo.push(Step {
            changes,
            group: None,
        });
        self.revision += 1;
        Some(step.changes)
    }

    pub(crate) fn redo(&mut self) -> Option<Vec<Change>> {
        let history = Rc::make_mut(&mut self.history);
        let step = history.redo.pop()?;
        let changes = change::apply_all(Rc::make_mut(&mut self.canvas), &step.changes);
        history.undo.push(Step {
            changes,
            group: None,
        });
        self.revision += 1;
        Some(step.changes)
    }

    pub fn can_undo(&self) -> bool {
        !self.history.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.history.redo.is_empty()
    }

    pub(crate) fn last_group(&self) -> Option<u64> {
        self.history.undo.last().and_then(|step| step.group)
    }

    pub(crate) fn next_group(&mut self) -> u64 {
        let history = Rc::make_mut(&mut self.history);
        history.groups += 1;
        history.groups
    }
}
