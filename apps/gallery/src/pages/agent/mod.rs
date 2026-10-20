//! The Agent tab.

pub mod activity;
pub mod avatar;
pub mod browser;
pub mod diff;
pub mod orbs;
#[cfg(not(target_family = "wasm"))]
pub mod terminal;
pub mod transcript;

use gpui::{AnyElement, Context, Entity, prelude::*};

use crate::Gallery;

/// What this tab's pages hold between frames, one entity per page.
pub(crate) struct State {
    activity: Entity<activity::Activity>,
    tool_calls: Entity<activity::ToolCalls>,
    composer: Entity<activity::Composer>,
    transcript: Entity<transcript::Transcript>,
    diff: Entity<diff::Diff>,
    #[cfg(not(target_family = "wasm"))]
    terminal: Entity<terminal::Terminal>,
    orbs: Entity<orbs::Orbs>,
    avatar: Entity<avatar::Avatars>,
    browser: Entity<browser::Browser>,
}

impl State {
    pub(crate) fn new(cx: &mut Context<Gallery>) -> Self {
        Self {
            activity: cx.new(activity::Activity::new),
            tool_calls: cx.new(|_| activity::ToolCalls::default()),
            composer: cx.new(activity::Composer::new),
            transcript: cx.new(transcript::Transcript::new),
            diff: cx.new(|_| diff::Diff),
            #[cfg(not(target_family = "wasm"))]
            terminal: cx.new(terminal::Terminal::new),
            orbs: cx.new(orbs::Orbs::new),
            avatar: cx.new(avatar::Avatars::new),
            browser: cx.new(browser::Browser::new),
        }
    }
}

impl Gallery {
    pub(crate) fn agent(&mut self, key: &str) -> Option<AnyElement> {
        let state = &self.agent;
        Some(match key {
            "agent-activity" => state.activity.clone().into_any_element(),
            "agent-tools" => state.tool_calls.clone().into_any_element(),
            "agent-composer" => state.composer.clone().into_any_element(),
            "agent-transcript" => state.transcript.clone().into_any_element(),
            "agent-diff" => state.diff.clone().into_any_element(),
            #[cfg(not(target_family = "wasm"))]
            "agent-terminal" => state.terminal.clone().into_any_element(),
            "agent-orbs" => state.orbs.clone().into_any_element(),
            "agent-avatar" => state.avatar.clone().into_any_element(),
            "browser" => state.browser.clone().into_any_element(),
            _ => return None,
        })
    }
}
