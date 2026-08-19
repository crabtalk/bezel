//! Patterns — composed screens rather than primitives, shadcn's "blocks".
//!
//! A page here is not a component demo: it is an app, small enough to read
//! whole and complete enough to copy. Its rail row points at *this* source
//! rather than into `crates/`, because the pattern is the code you take.

pub mod agent;
pub mod avatar;
pub mod diff;
pub mod document;
pub mod orbs;
pub mod samples;
pub mod syntax;
#[cfg(not(target_family = "wasm"))]
pub mod terminal;
pub mod transcript;
