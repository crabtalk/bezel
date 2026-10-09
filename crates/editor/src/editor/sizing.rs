//! A painted block's height, dragged off the handle along its bottom edge.
//!
//! The blocks are the ones a painted fence or an embed card stands for, which
//! [`markdown::BlockLayouts::painted_bounds`] answers for. The grip that
//! starts a drag is the block's own, painted by `markdown`, which calls
//! [`markdown::FenceHost::resize`]. The height is the
//! whole block's, in whole pixels, and the document only learns it on release,
//! the same way an image's width waits for the drop.

use gpui::{Context, Pixels, px};
use markdown::{BlockKind, Form};

use crate::{editor::Editor, history::EditKind};

/// The shortest a block can be dragged to.
const MIN_HEIGHT: f32 = 120.0;

/// A height drag in flight: the block, where the press was and the block's
/// height then, and the height it holds now.
#[derive(Clone, Copy)]
pub(super) struct Sizing {
    pub ix: usize,
    from: Pixels,
    start: Pixels,
    pub live: Option<u32>,
}

/// The height `kind` states, when it is a block that can state one.
fn stated(kind: &BlockKind) -> Option<Option<u32>> {
    match kind {
        BlockKind::Code { height, .. } => Some(*height),
        BlockKind::Bookmark {
            form: Form::Embed(height),
            ..
        } => Some(*height),
        _ => None,
    }
}

impl Editor {
    /// The live height the renderer paints the dragged block at.
    pub(super) fn held_height(&self) -> Option<(usize, u32)> {
        let sizing = self.sizing?;
        Some((sizing.ix, sizing.live?))
    }

    /// Follow the pointer with the block being sized. `false` when none is.
    pub(super) fn drag_height(&mut self, y: Pixels, cx: &mut Context<Self>) -> bool {
        let Some(sizing) = self.sizing.as_mut() else {
            return false;
        };
        let height = (sizing.start + (y - sizing.from)).max(px(MIN_HEIGHT));
        sizing.live = Some(height.as_f32().round() as u32);
        cx.notify();
        true
    }

    /// End a height drag, writing the height down. `false` when none was
    /// running.
    pub(super) fn drop_height(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(sizing) = self.sizing.take() else {
            return false;
        };
        let Some(height) = sizing.live else {
            cx.notify();
            return true;
        };
        let current = self
            .doc
            .blocks
            .get(sizing.ix)
            .and_then(|block| stated(&block.kind));
        if current != Some(Some(height)) {
            self.edit(EditKind::Structure, cx, |this| {
                match this
                    .doc
                    .blocks
                    .get_mut(sizing.ix)
                    .map(|block| &mut block.kind)
                {
                    Some(BlockKind::Code { height: at, .. }) => *at = Some(height),
                    Some(BlockKind::Bookmark { form, .. }) => *form = Form::Embed(Some(height)),
                    _ => {}
                }
                vec![]
            });
            // Made where the grip was, not at the caret: the reveal `edit`
            // asked for would scroll to a caret that can be pages away.
            self.reveal = None;
        } else {
            cx.notify();
        }
        true
    }

    /// Start a height drag off the grip of the painted block at `ix`, pressed
    /// at window `y`.
    pub(super) fn start_height(&mut self, ix: usize, y: Pixels, cx: &mut Context<Self>) {
        let Some(painted) = self.layouts.painted_bounds(ix) else {
            return;
        };
        self.sizing = Some(Sizing {
            ix,
            from: y,
            start: painted.size.height,
            live: None,
        });
        cx.notify();
    }
}
