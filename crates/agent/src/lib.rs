//! Agent-related UI, apart from the component library: what a chat client
//! draws around the model, not the widgets it composes.
//!
//! Two pieces, opposite in nature. [`orbs`] is stateful — the model's working
//! state, animated by its own clock, reading the theme. [`avatar`] is pure —
//! the same name always draws the same face, so identity survives a reload;
//! its colors come from the name, never from the environment.
//!
//! ```ignore
//! agent::avatar("Sara").w(px(48.)).h(px(48.))
//! ```

pub mod avatar;
pub mod orbs;

/// `agent::avatar("Sara")` — the blob avatar, re-exported at the root.
pub use avatar::avatar;
