//! Thinking orbs — dotted loading indicators for AI & agent UIs, ported from
//! gpui-thinking-orbs (MIT), itself a port of Jakub Antalik's thinking-orbs.
//!
//! Twelve hand-tuned animated states, four size presets, monochrome ink on a
//! transparent canvas. The animation engine is pure Rust; only the paint path
//! talks to gpui.
//!
//! ```ignore
//! cx.new(|_| Orb::new().state(OrbState::Searching).size(OrbSize::Avatar))
//! ```
//!
//! Style flows through the environment like the rest of bezel: [`OrbTheme::Auto`]
//! resolves ink against the installed [`bezel_theme::Appearance`] at paint time.
//! The engine is the public `engine` module below — the "advanced" tier for
//! custom paint loops.

mod orb;
mod paint;
mod presets;
mod types;

pub mod engine;

pub use orb::{DEFAULT_TARGET_FPS, Orb, orb_element};
pub use presets::{Resolved, resolve_preset};
pub use types::{ModeKey, OrbSize, OrbState, OrbTheme};
