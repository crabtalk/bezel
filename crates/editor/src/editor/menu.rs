//! The floating chrome: the gutter handle, the drop indicator, and the two
//! menus.
//!
//! All four are placed from positions `markdown::BlockLayouts` recorded as it
//! painted, so none of them can drift from the text it points at.

use gpui::{AnyElement, Context, CursorStyle, MouseButton, SharedString, div, prelude::*, px};
use markdown::BlockKind;
use motion::{Fade, Painter};
use theme::Theme;

use crate::editor::{Editor, HANDLE_GUTTER, HANDLE_SIZE};

/// How far the language chip reaches past the word it wraps.
const CHIP_PAD_X: f32 = 6.0;
const CHIP_PAD_Y: f32 = 3.0;

/// Selector the interaction tests look the painted menu up by.
pub const SLASH_MENU: &str = "slash-menu";

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
                    cx.listener(move |this, _: &gpui::MouseDownEvent, _, cx| {
                        this.press_claimed = true;
                        this.lifted = Some((ix, ix));
                        // The menu belongs to the release. Opened here it would
                        // occlude the very moves a drag downwards is made of,
                        // and the drop target would never leave the block it
                        // started on.
                        this.block_menu = None;
                        cx.notify();
                    }),
                )
                .into_any_element(),
        )
    }

    /// The line showing where a lifted block — or a file dragged in from
    /// outside — would land.
    pub(super) fn drop_indicator(&self, theme: &Theme) -> Option<AnyElement> {
        let (from, to) = match self.lifted.filter(|(from, to)| from != to) {
            Some(lifted) => lifted,
            // A file always lands under the block it is over, so it is a drag
            // that only ever moves downwards.
            None => self.dropping.map(|to| (to, to + 1))?,
        };
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

    /// The click target over a fence's header, where its language sits.
    ///
    /// The header paints the name; this only takes the press. A renderer holds
    /// a `&Doc` and cannot change one, which is why the copy button can live
    /// down there and a language picker cannot.
    pub(super) fn language_chip(
        &self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        // Whichever fence the reader is at: the one under the pointer, else the
        // one the caret is in.
        let ix = [self.hovered, Some(self.cursor().block)]
            .into_iter()
            .flatten()
            .find(|ix| {
                matches!(
                    self.doc.blocks.get(*ix).map(|b| &b.kind),
                    Some(BlockKind::Code { .. })
                )
            })?;
        // The label's box, grown by the chip's padding — so the wash is around
        // the word and nothing else, whatever the word is.
        let bounds = self.layouts.language_bounds(ix)?;
        let anchor = gpui::point(
            bounds.origin.x - px(CHIP_PAD_X),
            bounds.origin.y + bounds.size.height + px(CHIP_PAD_Y),
        );
        Some(
            div()
                .id("language-chip")
                .absolute()
                .left(bounds.origin.x - self.origin.x - px(CHIP_PAD_X))
                .top(bounds.origin.y - self.origin.y - px(CHIP_PAD_Y))
                .w(bounds.size.width + px(2.0 * CHIP_PAD_X))
                .h(bounds.size.height + px(2.0 * CHIP_PAD_Y))
                .rounded(px(4.0))
                .cursor(CursorStyle::PointingHand)
                .hover(|el| el.bg(theme.ink(0.06)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.language_menu = Some((ix, anchor));
                    cx.notify();
                }))
                .into_any_element(),
        )
    }

    /// The languages the installed highlighter knows, at the header that asked.
    pub(super) fn language_menu(
        &self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let view = Painter::of(cx);
        let (ix, at) = self.language_menu?;
        let Some(BlockKind::Code { language, .. }) = self.doc.blocks.get(ix).map(|b| &b.kind)
        else {
            return None;
        };
        let current = language.clone();
        let row = |label: SharedString, tag: Option<String>, lit: bool| {
            ui::popover::menu_row(theme, lit, Fade::new(view, format!("lang-{label}")))
                .id(SharedString::from(format!("lang-row-{label}")))
                .child(label.clone())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.language_menu = None;
                    this.set_language(ix, tag.clone(), cx);
                }))
        };
        let plain = row(
            markdown::render::PLAIN_LANGUAGE.into(),
            None,
            current.is_none(),
        );
        let rows: Vec<AnyElement> = markdown::languages(cx)
            .to_vec()
            .into_iter()
            .map(|name| {
                let lit = current.as_deref() == Some(name.as_ref());
                row(name.clone(), Some(name.to_string()), lit).into_any_element()
            })
            .collect();
        Some(ui::popover::menu_at(
            "language-menu",
            at,
            ui::popover::popover_card(theme)
                .w(px(150.0))
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    this.language_menu = None;
                    cx.notify();
                }))
                .child(
                    div()
                        .id("language-menu-rows")
                        .max_h(px(280.0))
                        .overflow_y_scroll()
                        .child(plain)
                        .children(rows),
                )
                .into_any_element(),
            None,
        ))
    }

    /// Turn into / Duplicate / Delete, at the handle that opened it.
    pub(super) fn block_menu(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let view = Painter::of(cx);
        let (ix, at) = self.block_menu?;
        let turns = crate::slash::items();
        let rows = turns.into_iter().map(|(label, kind)| {
            ui::popover::menu_row(theme, false, Fade::new(view, format!("turn-{label}")))
                .id(SharedString::from(format!("turn-row-{label}")))
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.block_menu = None;
                    this.set_block(ix, kind.clone(), cx);
                }))
        });
        let action = |label: &'static str, run: fn(&mut Self, usize, &mut Context<Self>)| {
            ui::popover::menu_row(theme, false, Fade::new(view, format!("block-{label}")))
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
            ui::popover::popover_card(theme)
                .w(px(190.0))
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    this.block_menu = None;
                    cx.notify();
                }))
                .child(
                    div()
                        .id("block-menu-rows")
                        .max_h(px(320.0))
                        .overflow_y_scroll()
                        .child(ui::popover::menu_heading(theme, "Turn into"))
                        .children(rows)
                        .child(ui::popover::menu_heading(theme, "Block"))
                        .child(action("Duplicate", |this, ix, cx| {
                            this.duplicate_block(ix, cx)
                        }))
                        .child(action("Delete", |this, ix, cx| this.remove_block(ix, cx))),
                )
                .into_any_element(),
            None,
        ))
    }

    /// What a pasted URL could be, under the block it landed in.
    ///
    /// Anchored at the block's start rather than at the caret: the caret is
    /// past the end of a URL, which is as far right as a line goes, and a menu
    /// hanging off there points at nothing.
    pub(super) fn paste_menu(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let view = Painter::of(cx);
        let pasted = self.pasted.as_ref()?;
        let (point, line_height) = self.layouts.position(pasted.at)?;
        let rows = pasted.rows.iter().enumerate().map(|(row, &choice)| {
            let label = choice.label();
            ui::popover::menu_row(theme, row == pasted.active, Fade::new(view, label))
                .id(SharedString::from(format!("paste-row-{label}")))
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| this.confirm_paste(choice, cx)))
        });
        Some(ui::popover::menu_at(
            "paste-menu",
            gpui::point(point.x, point.y + line_height),
            ui::popover::popover_card(theme)
                .w(px(180.0))
                // Clicking away is `Dismiss`, which is a real answer: the link
                // is already in the block and stays there.
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    this.pasted = None;
                    cx.notify();
                }))
                .children(rows)
                .into_any_element(),
            None,
        ))
    }

    /// The menu, anchored under the `/` that opened it.
    ///
    /// The anchor comes from the same layout the caret paints against, so it
    /// costs nothing beyond a lookup and it cannot drift from the text.
    pub(super) fn slash_menu(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let view = Painter::of(cx);
        let slash = self.slash.as_ref()?;
        let (point, line_height) = self.layouts.position(slash.at)?;
        let items = crate::slash::items();
        let reduce_motion = cx.reduce_motion();
        // The `.id` is not optional: `menu_row` registers its hover fade
        // imperatively and needs a stateful element to hang it on, so a row
        // without one neither highlights nor clicks.
        let rows = slash
            .filter
            .filtered()
            .iter()
            .enumerate()
            .map(|(row, &ix)| {
                let kind = items[ix].1.clone();
                ui::popover::menu_row(
                    theme,
                    Some(row) == slash.filter.active(),
                    Fade::new(view, format!("slash-{ix}")),
                )
                .id(SharedString::from(format!("slash-row-{ix}")))
                .child(items[ix].0.clone())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.confirm_slash(Some(kind.clone()), cx);
                }))
            });
        Some(ui::popover::menu_at(
            "slash-menu",
            gpui::point(point.x, point.y + line_height),
            ui::popover::popover_card(theme)
                // Compiles to nothing outside a test build. It is here because
                // the menu's state opening and the menu *painting* are two
                // different things, and the bug that shipped was the second one
                // failing while the first looked fine.
                .debug_selector(|| SLASH_MENU.to_string())
                .w(px(200.0))
                .relative()
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    this.slash = None;
                    cx.notify();
                }))
                .child(
                    div()
                        .id("slash-rows")
                        .max_h(px(280.0))
                        .overflow_y_scroll()
                        .track_scroll(&slash.scroll)
                        .children(rows),
                )
                .child(ui::scroll::transient(
                    "slash-bar",
                    &slash.scroll,
                    &slash.bar,
                    reduce_motion,
                ))
                .into_any_element(),
            None,
        ))
    }
}
