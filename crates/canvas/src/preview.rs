use crate::{
    change::{self, Change},
    model::Canvas,
};
use std::rc::Rc;

pub(crate) struct Preview {
    source: Rc<Canvas>,
    pub canvas: Canvas,
    restore: Vec<Change>,
}

impl Preview {
    pub fn new(source: Rc<Canvas>) -> Self {
        Self {
            canvas: (*source).clone(),
            source,
            restore: Vec::new(),
        }
    }

    pub fn reset(&mut self, source: &Rc<Canvas>) {
        if !Rc::ptr_eq(&self.source, source) {
            *self = Self::new(source.clone());
            return;
        }
        change::apply_all(&mut self.canvas, &std::mem::take(&mut self.restore));
    }

    pub fn apply(&mut self, change: &Change) {
        let restore = change::apply(&mut self.canvas, change);
        self.restore.splice(0..0, restore);
    }
}
