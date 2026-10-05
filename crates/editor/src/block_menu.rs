//! The menu a block's gutter handle opens: the rows the app installed with
//! [`crate::AppExt::set_block_menu_items`], or [`defaults`] until it does.

use std::rc::Rc;

use gpui::{App, Global};
use ui::icons::glyph;

use crate::slash::{SlashAction, SlashAt, SlashItem, SlashRow};

/// The slash menu's default blocks under "Turn into", then Duplicate and
/// Delete under "Block".
pub fn defaults() -> Vec<SlashItem> {
    let mut items = vec![SlashItem::Heading("Turn into".into())];
    items.extend(crate::slash::turns(crate::slash::defaults()));
    items.push(SlashItem::Heading("Block".into()));
    items.push(SlashItem::Row(SlashRow {
        label: "Duplicate".into(),
        icon: Some(glyph::CopyPlus.into()),
        action: SlashAction::Run(Rc::new(|at: SlashAt, _, cx| {
            at.editor
                .update(cx, |editor, cx| editor.duplicate_block(at.block, cx))
                .ok();
        })),
    }));
    items.push(SlashItem::Row(SlashRow {
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
pub(crate) struct Installed(pub Vec<SlashItem>);

impl Global for Installed {}

/// The items the app installed, or [`defaults`].
pub(crate) fn installed(cx: &App) -> Vec<SlashItem> {
    cx.try_global::<Installed>()
        .map_or_else(defaults, |Installed(items)| items.clone())
}
