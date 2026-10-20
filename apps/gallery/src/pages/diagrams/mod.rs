//! The Diagrams tab: a canvas, charts and a Mermaid playground drawn live, and
//! every diagram type `bezel-mermaid` draws.

pub mod charts;
pub mod mermaid;
pub mod mindmap;
pub mod playground;

use gpui::{AnyElement, Context, Entity, Window, prelude::*};

use crate::Gallery;

/// What this tab's pages hold between frames.
pub(crate) struct State {
    pub(crate) mindmap: Entity<mindmap::CanvasDemo>,
    charts: Entity<charts::Charts>,
    playground: Entity<playground::Playground>,
    mermaid: mermaid::State,
}

impl State {
    pub(crate) fn new(cx: &mut Context<Gallery>) -> Self {
        Self {
            mindmap: cx.new(mindmap::CanvasDemo::new),
            charts: cx.new(charts::Charts::new),
            playground: cx.new(playground::Playground::new),
            mermaid: mermaid::State::new(cx),
        }
    }
}

impl Gallery {
    pub(crate) fn diagrams(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        match key {
            "canvas" => Some(self.diagrams.mindmap.clone().into_any_element()),
            "charts" => Some(self.diagrams.charts.clone().into_any_element()),
            "playground" => Some(self.diagrams.playground.clone().into_any_element()),
            _ => self.mermaid(key, window, cx),
        }
    }
}
