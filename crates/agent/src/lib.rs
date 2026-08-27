//! Agent-related UI, apart from the component library: what a chat client
//! draws around the model, not the widgets it composes.
//!
//! Two pieces. [`orbs`] is the model's working state, animated by its own
//! clock. [`avatar`] is a face: a silhouette generated from a preset, a name or
//! [`avatar::Shape::random`], painted in the theme's colors — smooth at any
//! size, or sampled onto an 8×8 grid by [`mascot`] for a list row.
//!
//! ```ignore
//! agent::avatar(Face::from("Sara").pose(t)).w(px(48.)).h(px(48.))
//! ```

pub mod avatar;
pub mod orbs;

pub use avatar::{Face, Pose, avatar, mascot};
