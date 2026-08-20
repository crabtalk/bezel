//! The floating chrome: the gutter handle, the drop indicator, and the two
//! menus.
//!
//! All four are placed from positions `markdown::BlockLayouts` recorded as it
//! painted, so none of them can drift from the text it points at.

use gpui::{AnyElement, Context, CursorStyle, MouseButton, SharedString, div, prelude::*, px};
use theme::Theme;

use crate::editor::{Editor, HANDLE_GUTTER, HANDLE_SIZE};

impl Editor {
    /// The gutter handle, on the block under the pointer.
    ///
    /// One handle rather than one per block: only the hovered block shows it,
    /// so a single element placed from the recorded frames does the whole job
    /// and the renderer stays clear of editor concerns.
    pub(super) fn handle(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let ix = self.lifted.map(|(from, _)| from).or(self.hovered)?;
        let bounds = self.layouts.block_bounds(ix)?;
        Some(
            div()
                .id("block-handle")
                .absolute()
                .left(bounds.origin.x - self.origin.x - px(HANDLE_GUTTER))
                .top(bounds.origin.y - self.origin.y)
                .w(px(HANDLE_SIZE))
                .h(px(HANDLE_SIZE))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(4.0))
                .cursor(CursorStyle::OpenHand)
                .text_size(px(12.0))
                .text_color(theme.text_faint)
                .hover(|el| el.bg(theme.ink(0.08)).text_color(theme.text_muted))
                .child("⠿")
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                        this.handle_pressed = true;
                        this.lifted = Some((ix, ix));
                        // A press that never moves is a click and leaves the
                        // menu open; the first drag move clears it.
                        this.block_menu = Some((ix, event.position));
                        cx.notify();
                    }),
                )
                .into_any_element(),
        )
    }

    /// The line showing where a lifted block would land.
    pub(super) fn drop_indicator(&self, theme: &Theme) -> Option<AnyElement> {
        let (from, to) = self.lifted.filter(|(from, to)| from != to)?;
        let bounds = self.layouts.block_bounds(to)?;
        // Above the target when moving up, below it when moving down — which
        // is where the block actually ends up.
        let y = if to < from {
            bounds.origin.y
        } else {
            bounds.origin.y + bounds.size.height
        };
        Some(
            div()
                .absolute()
                .left(px(0.0))
                .top(y - self.origin.y - px(1.0))
                .w_full()
                .h(px(2.0))
                .rounded(px(1.0))
                .bg(theme.accent)
                .into_any_element(),
        )
    }

    /// Turn into / Duplicate / Delete, at the handle that opened it.
    pub(super) fn block_menu(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (ix, at) = self.block_menu?;
        let turns = crate::slash::items();
        let rows = turns.into_iter().map(|(label, kind)| {
            ui::popover::menu_row(theme, false, SharedString::from(format!("turn-{label}")))
                .id(SharedString::from(format!("turn-row-{label}")))
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.block_menu = None;
                    this.set_block(ix, kind.clone(), cx);
                }))
        });
        let action = |label: &'static str, run: fn(&mut Self, usize, &mut Context<Self>)| {
            ui::popover::menu_row(theme, false, SharedString::from(format!("block-{label}")))
                .id(SharedString::from(format!("block-row-{label}")))
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.block_menu = None;
                    run(this, ix, cx);
                }))
        };
        Some(ui::popover::menu_at(
            "block-menu",
            at,
            div()
                .w(px(190.0))
                .max_h(px(320.0))
                .overflow_hidden()
                .child(ui::popover::menu_heading(theme, "Turn into"))
                .children(rows)
                .child(ui::popover::menu_heading(theme, "Block"))
                .child(action("Duplicate", |this, ix, cx| {
                    this.duplicate_block(ix, cx)
                }))
                .child(action("Delete", |this, ix, cx| this.remove_block(ix, cx)))
                .into_any_element(),
            None,
        ))
    }

    /// The menu, anchored under the `/` that opened it.
    ///
    /// The anchor comes from the same layout the caret paints against, so it
    /// costs nothing beyond a lookup and it cannot drift from the text.
    pub(super) fn slash_menu(&self, theme: &Theme) -> Option<AnyElement> {
        let slash = self.slash.as_ref()?;
        let (point, line_height) = self.layouts.position(slash.at)?;
        let items = crate::slash::items();
        let rows = slash
            .filter
            .filtered()
            .iter()
            .enumerate()
            .map(|(row, &ix)| {
                ui::popover::menu_row(
                    theme,
                    Some(row) == slash.filter.active(),
                    SharedString::from(format!("slash-{ix}")),
                )
                .child(items[ix].0.clone())
            });
        Some(ui::popover::menu_at(
            "slash-menu",
            gpui::point(point.x, point.y + line_height),
            div()
                .w(px(200.0))
                .max_h(px(280.0))
                .overflow_hidden()
                .children(rows)
                .into_any_element(),
            None,
        ))
    }
}
