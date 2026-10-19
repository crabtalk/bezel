//! The Editor tab.

pub mod dialect;
pub mod document;
pub mod editing;
pub mod ribbon;
pub mod selectable;
pub mod syntax;

use gpui::{AnyElement, Context, Entity, prelude::*};
use theme::Theme;

use crate::{Gallery, hint, stack};

/// What this tab's pages hold between frames, one entity per page.
pub(crate) struct State {
    document: Entity<document::Document>,
    /// Prose a reader can drag over, which owns the selection the way any host
    /// of `markdown::render::selectable` has to.
    selectable: Entity<selectable::Selectable>,
    editing: Entity<editing::EditorDemo>,
    ribbon: Entity<ribbon::RibbonDemo>,
    dialect: Entity<dialect::Dialect>,
    syntax: Entity<syntax::Syntax>,
}

impl State {
    pub(crate) fn new(cx: &mut Context<Gallery>) -> Self {
        Self {
            document: cx.new(document::Document::new),
            selectable: cx.new(selectable::Selectable::new),
            editing: cx.new(editing::EditorDemo::new),
            ribbon: cx.new(ribbon::RibbonDemo::new),
            dialect: cx.new(dialect::Dialect::new),
            syntax: cx.new(syntax::Syntax::new),
        }
    }
}

impl Gallery {
    pub(crate) fn editor(&mut self, key: &str, cx: &mut Context<Self>) -> Option<AnyElement> {
        let state = &self.editor;
        Some(match key {
            "document" => state.document.clone().into_any_element(),
            "selectable-text" => stack()
                .child(hint(
                    Theme::of(cx),
                    "Press in the prose and drag. The selection is painted by \
                     the renderer the editor uses and resolves against the same \
                     layouts — what the library adds is the gesture, and \
                     nothing else. Which document holds the selection, and what \
                     copying means, stay the screen's.",
                ))
                .child(state.selectable.clone())
                .into_any_element(),
            "editor" => state.editing.clone().into_any_element(),
            "ribbon" => state.ribbon.clone().into_any_element(),
            "markdown" => state.dialect.clone().into_any_element(),
            "syntax" => state.syntax.clone().into_any_element(),
            _ => return None,
        })
    }
}
