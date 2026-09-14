//! Bounded snapshot storage. Callers decide which edits form a group.

use std::collections::VecDeque;

pub struct SnapshotHistory<S> {
    undo: VecDeque<S>,
    redo: Vec<S>,
    limit: usize,
}

impl<S> SnapshotHistory<S> {
    pub fn new(limit: usize) -> Self {
        Self {
            undo: VecDeque::new(),
            redo: Vec::new(),
            limit,
        }
    }

    pub fn set_limit(&mut self, limit: usize) {
        self.limit = limit;
        self.trim();
        let excess = self.redo.len().saturating_sub(limit);
        self.redo.drain(..excess);
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Every edit discards redo. `None` continues the caller's current group.
    pub fn record(&mut self, before: Option<S>) {
        self.redo.clear();
        if let Some(before) = before {
            self.undo.push_back(before);
            self.trim();
        }
    }

    pub fn undo(&mut self, current: impl FnOnce() -> S) -> Option<S> {
        let previous = self.undo.pop_back()?;
        self.redo.push(current());
        Some(previous)
    }

    pub fn redo(&mut self, current: impl FnOnce() -> S) -> Option<S> {
        let next = self.redo.pop()?;
        self.undo.push_back(current());
        self.trim();
        Some(next)
    }

    fn trim(&mut self) {
        while self.undo.len() > self.limit {
            self.undo.pop_front();
        }
    }
}
