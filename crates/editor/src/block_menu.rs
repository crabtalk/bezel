//! The menu a block's gutter handle opens: the rows the app installed with
//! [`crate::AppExt::set_block_menu_items`], or [`defaults`] until it does.

use std::rc::Rc;

use gpui::{App, Global, SharedString};
use ui::icons::glyph;

use crate::slash::{SlashAction, SlashAt, SlashRow};

/// An item of the block menu, in the order it is listed.
#[derive(Clone)]
pub enum BlockMenuItem {
    /// A section heading over the rows after it.
    Heading(SharedString),
    Row(SlashRow),
}

/// The slash menu's default blocks under "Turn into", then Duplicate and
/// Delete under "Block".
pub fn defaults() -> Vec<BlockMenuItem> {
    let mut items = vec![BlockMenuItem::Heading("Turn into".into())];
    items.extend(
        crate::slash::turns(crate::slash::defaults())
            .into_iter()
            .map(BlockMenuItem::Row),
    );
    items.push(BlockMenuItem::Heading("Block".into()));
    items.push(BlockMenuItem::Row(SlashRow {
        label: "Duplicate".into(),
        icon: Some(glyph::CopyPlus.into()),
        action: SlashAction::Run(Rc::new(|at: SlashAt, _, cx| {
            at.editor
                .update(cx, |editor, cx| editor.duplicate_block(at.block, cx))
                .ok();
        })),
    }));
    items.push(BlockMenuItem::Row(SlashRow {
        label: "Delete".into(),
        icon: Some(glyph::Trash.into()),
        action: SlashAction::Run(Rc::new(|at: SlashAt, _, cx| {
            at.editor
                .update(cx, |editor, cx| editor.remove_block(at.block, cx))
                .ok();
        })),
    }));
    items
}

/// What the app installed.
pub(crate) struct Installed(pub Vec<BlockMenuItem>);

impl Global for Installed {}

/// The items the app installed, or [`defaults`].
pub(crate) fn installed(cx: &App) -> Vec<BlockMenuItem> {
    cx.try_global::<Installed>()
        .map_or_else(defaults, |Installed(items)| items.clone())
}
