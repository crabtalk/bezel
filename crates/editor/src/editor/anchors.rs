//! Anchors: ids on ranges that follow the words through edits.

use super::*;

impl Editor {
    /// The comment ranges, mapped up to date with the document.
    ///
    /// A range that reads [`Anchor::detached`] lost the words it pointed at.
    /// It is kept rather than dropped, because whether that means "outdated" or
    /// "resolved" is the app's question.
    pub fn anchors(&self) -> &[Anchor] {
        &self.anchors
    }

    /// Hand over the whole list — the app keeps the threads, this keeps their
    /// ranges. One entry point rather than add/remove/update, since the app is
    /// already holding the list that decides all three.
    pub fn set_anchors(&mut self, anchors: Vec<Anchor>, cx: &mut Context<Self>) {
        self.anchors = anchors;
        cx.notify();
    }

    /// The comment under a point in window coordinates — the space
    /// [`Self::anchor_bounds`] answers in and the press handler resolves in.
    ///
    /// The last match wins, so the newer of two overlapping ranges is the one a
    /// click opens.
    pub fn anchor_at(&self, at: gpui::Point<gpui::Pixels>) -> Option<AnchorId> {
        let at = self.layouts.hit(at)?;
        self.anchors
            .iter()
            .rfind(|anchor| {
                let (start, end) = anchor.range.ordered();
                !anchor.detached() && start <= at && at <= end
            })
            .map(|anchor| anchor.id)
    }

    /// Where to float a thread, mirroring [`Self::selection_bounds`].
    pub fn anchor_bounds(&self, id: AnchorId) -> Option<gpui::Bounds<gpui::Pixels>> {
        let anchor = self.anchors.iter().find(|anchor| anchor.id == id)?;
        let (point, line_height) = self.layouts.position(anchor.range.ordered().0)?;
        Some(gpui::Bounds::new(
            point,
            gpui::size(gpui::px(0.0), line_height),
        ))
    }

    /// The ranges the renderer washes, clamped because a block can change kind
    /// under an anchor and take its part with it.
    pub(super) fn annotations(&self) -> Vec<(Selection, Annotation)> {
        self.anchors
            .iter()
            .filter(|anchor| !anchor.detached())
            .map(|anchor| (anchor.range.clamp(&self.doc), anchor.state))
            .collect()
    }
}
