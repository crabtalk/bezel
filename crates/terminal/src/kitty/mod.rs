//! The kitty graphics protocol: commands, and the store that carries them
//! out.
//!
//! What lands here is the transmission half of the protocol — the bytes of an
//! image, assembled and stored under its id, and an animation's frames drawn
//! over each other as they arrive. Placements — where an image goes
//! on the grid, and which of them a delete takes off it — are the emulator's,
//! and painting is the view's.

use std::collections::HashMap;

pub use crate::placeholder::PLACEHOLDER;

/// The most one assembled image may carry, chunks included: 64 MiB, which is a
/// 4096x4096 image in RGBA with room over.
const MAX_IMAGE: usize = 64 << 20;

/// How many images are kept before the oldest is dropped. Kitty has a
/// storage quota in bytes; this is the same idea at the granularity the store
/// actually evicts at.
const MAX_IMAGES: usize = 64;

mod animation;
mod command;
mod store;

pub use command::*;
pub use store::*;
