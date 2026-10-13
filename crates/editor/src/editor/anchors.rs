//! Anchors: ids on ranges that follow the words through edits.

use super::*;

impl Editor {
    /// This editor's snapshot of [`crate::Document::anchors`].
    pub fn anchors(&self) -> &[Anchor] {
        &self.anchors
    }

    /// The comment under a point in window coordinates — the space
    /// [`Self::anchor_bounds`] answers in and the press handler resolves in.
    ///
    /// The last match wins, so the newer of two overlapping ranges is the one a
    /// click opens.
    pub fn anchor_at(&self, at: gpui::Point<gpui::Pixels>) -> Option<AnchorId> {
        let (at, _) = self.layouts.hit(at)?;
        self.anchors
            .iter()
            .rfind(|anchor| {
                let (start, end) = anchor.range.ordered();
                !anchor.detached() && start <= at && at <= end
            })
            .map(|anchor| anchor.id)
    }

    /// Whether the pointer is over an anchor marked [`Anchor::actionable`].
    pub(crate) fn over_actionable(&self) -> bool {
        self.over_anchor.is_some_and(|id| {
            self.anchors
                .iter()
                .any(|anchor| anchor.id == id && anchor.actionable)
        })
    }

    /// Where to float a thread, mirroring [`Self::selection_bounds`].
    pub fn anchor_bounds(&self, id: AnchorId) -> Option<gpui::Bounds<gpui::Pixels>> {
        let anchor = self.anchors.iter().find(|anchor| anchor.id == id)?;
        let (point, line_height) = self
            .layouts
            .position(anchor.range.ordered().0, Affinity::Downstream)?;
        Some(gpui::Bounds::new(
            point,
            gpui::size(gpui::px(0.0), line_height),
        ))
    }

    /// How far down the editor each anchor's first line sits, from the
    /// editor's own top, in document order. A detached anchor sits where its
    /// words were.
    ///
    /// Read from last frame's layout. An anchor whose line has not been built
    /// gets the top of its block, guessed until the block is measured; the
    /// editor repaints when a guess moves.
    pub fn anchor_tops(&self) -> Vec<(AnchorId, gpui::Pixels)> {
        let mut tops: Vec<_> = self
            .anchors
            .iter()
            .filter_map(|anchor| {
                let start = anchor.range.clamp(&self.doc).ordered().0;
                Some((start, anchor.id, self.layouts.top(start)? - self.origin.y))
            })
            .collect();
        tops.sort_by_key(|(start, ..)| *start);
        tops.into_iter().map(|(_, id, top)| (id, top)).collect()
    }

    /// Wash `washes` from the next frame on, over the anchors' own. They are
    /// positions in [`Self::doc`] as it stands, and are not mapped through
    /// edits: an app that keeps them current sets them again.
    pub fn set_washes(&mut self, washes: Vec<(Selection, Annotation)>, cx: &mut Context<Self>) {
        self.washes = washes;
        cx.notify();
    }

    pub fn washes(&self) -> &[(Selection, Annotation)] {
        &self.washes
    }

    /// The ranges the renderer washes, clamped because a block can change kind
    /// under an anchor and take its part with it.
    pub(super) fn annotations(&self) -> Vec<(Selection, Annotation)> {
        self.anchors
            .iter()
            .filter(|anchor| !anchor.detached())
            .map(|anchor| (anchor.range, anchor.state))
            .chain(self.washes.iter().copied())
            .map(|(range, state)| (range.clamp(&self.doc), state))
            .collect()
    }
}
