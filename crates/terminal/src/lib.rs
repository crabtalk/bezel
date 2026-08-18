//! The terminal component: an `alacritty_terminal`-backed emulator — bytes in,
//! grid out — and the gpui element that paints the grid against the theme.
//!
//! The PTY itself is the host's job: this crate owns the escape-sequence state
//! machine and the paint, nothing else.

pub mod emulator;
pub mod view;
