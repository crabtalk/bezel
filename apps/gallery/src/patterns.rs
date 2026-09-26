//! Patterns — composed screens rather than primitives, shadcn's "blocks".
//!
//! A page here is not a component demo: it is an app, small enough to read
//! whole and complete enough to copy. Its rail row points at *this* source
//! rather than into `crates/`, because the pattern is the code you take.

pub mod agent;
pub mod avatar;
pub mod browser;
pub mod canvas;
pub mod dialect;
pub mod diff;
pub mod document;
pub mod editor;
pub mod orbs;
pub mod ribbon;
pub mod samples;
pub mod selectable;
pub mod syntax;
#[cfg(not(target_family = "wasm"))]
pub mod terminal;
pub mod transcript;

use gpui::{Context, Entity, prelude::*};

use crate::Gallery;

/// What this group's demos hold between frames.
pub(crate) struct State {
    /// One field per pattern, because a pattern is a screen and owns a screen's
    /// worth of state. A component demo can keep its value or two up here
    /// beside the rest; thirteen of them cannot.
    pub(crate) activity: Entity<agent::Activity>,
    pub(crate) tool_calls: Entity<agent::ToolCalls>,
    pub(crate) agent_composer: Entity<agent::Composer>,
    pub(crate) transcript: Entity<transcript::Transcript>,
    pub(crate) diff: Entity<diff::Diff>,
    pub(crate) document: Entity<document::Document>,
    pub(crate) dialect: Entity<dialect::Dialect>,
    pub(crate) ribbon: Entity<ribbon::RibbonDemo>,
    /// Prose a reader can drag over, which owns the selection the way any host
    /// of `markdown::selectable` has to.
    pub(crate) selectable: Entity<selectable::Selectable>,
    pub(crate) editor: Entity<editor::EditorDemo>,
    pub(crate) canvas: Entity<canvas::CanvasDemo>,
    #[cfg(not(target_family = "wasm"))]
    pub(crate) terminal: Entity<terminal::Terminal>,
    pub(crate) browser: Entity<browser::Browser>,
    pub(crate) orbs: Entity<orbs::Orbs>,
    pub(crate) syntax: Entity<syntax::Syntax>,
    pub(crate) avatar: Entity<avatar::Avatars>,
}

impl State {
    pub(crate) fn new(cx: &mut Context<Gallery>) -> Self {
        Self {
            activity: cx.new(agent::Activity::new),
            tool_calls: cx.new(|_| agent::ToolCalls::default()),
            agent_composer: cx.new(agent::Composer::new),
            transcript: cx.new(transcript::Transcript::new),
            diff: cx.new(|_| diff::Diff),
            document: cx.new(document::Document::new),
            dialect: cx.new(dialect::Dialect::new),
            ribbon: cx.new(ribbon::RibbonDemo::new),
            selectable: cx.new(selectable::Selectable::new),
            editor: cx.new(editor::EditorDemo::new),
            canvas: cx.new(canvas::CanvasDemo::new),
            #[cfg(not(target_family = "wasm"))]
            terminal: cx.new(terminal::Terminal::new),
            browser: cx.new(browser::Browser::new),
            orbs: cx.new(orbs::Orbs::new),
            syntax: cx.new(syntax::Syntax::new),
            avatar: cx.new(avatar::Avatars::new),
        }
    }
}
