//! Application policy for clipboard paste. File drops keep their own path.

use std::path::Path;

use gpui::{App, ClipboardItem, Entity, Global};

use crate::{Editor, Mode};

/// The destination snapshot; the editor entity is being updated during the call.
#[derive(Clone, Copy, Debug)]
pub struct PasteContext<'a> {
    pub mode: Mode,
    /// Whether the selection lies within one fence, including source mode.
    pub in_fence: bool,
    pub base: Option<&'a Path>,
}

/// Content applied through normal editing and undo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PasteContent {
    Literal(String),
    /// Uses normal text-paste rules: parsed with the editor's marks in rich
    /// text, literal in a fence or source mode, with normal URL handling.
    Markdown(String),
}

/// Called once before default clipboard handling. `None` keeps that behavior.
/// The entity identifies the editor; do not read or update it during the call.
/// Use the snapshot and `cx.image_store()` when calling `ImageStore::keep`.
pub type PasteHandler = fn(
    item: &ClipboardItem,
    editor: &Entity<Editor>,
    destination: PasteContext<'_>,
    cx: &mut App,
) -> Option<PasteContent>;

pub(crate) struct Installed(pub PasteHandler);

impl Global for Installed {}
