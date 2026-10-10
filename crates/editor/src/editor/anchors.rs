//! Anchors: ids on ranges that follow the words through edits.

use super::*;
use crate::anchor::SourceRanges;

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
    ///
    /// Ranges are positions in [`Self::doc`] as it stands: the source fence in
    /// [`Mode::Source`], the blocks otherwise.
    pub fn set_anchors(&mut self, anchors: Vec<Anchor>, cx: &mut Context<Self>) {
        self.anchors = anchors;
        cx.notify();
    }

    /// The markdown to save, and where each anchor's ends land in it, in byte
    /// offsets, in the order of [`Self::anchors`]. A range with an end the
    /// markdown has no place for comes out `None`.
    ///
    /// The string is [`Self::source`] but for whitespace a placed end keeps
    /// from being trailing; the ranges index this string, so it is the one to
    /// write beside them.
    pub fn source_with_anchors(&self) -> (String, SourceRanges) {
        let ends: Vec<_> = self
            .anchors
            .iter()
            .flat_map(|anchor| {
                let (start, end) = anchor.range.ordered();
                [start, end]
            })
            .collect();
        let (source, offsets) = match self.mode {
            Mode::Blocks => markdown::serialize_at_many(&self.doc, &ends, &self.marks),
            Mode::Source => {
                let source = self.source_text().to_string();
                let offsets = ends
                    .iter()
                    .map(|at| (at.offset <= source.len()).then_some(at.offset))
                    .collect();
                (source, offsets)
            }
        };
        let ranges = self
            .anchors
            .iter()
            .enumerate()
            .map(|(ix, anchor)| {
                let range = offsets[2 * ix].zip(offsets[2 * ix + 1]);
                (anchor.id, range.map(|(start, end)| start..end))
            })
            .collect();
        (source, ranges)
    }

    /// [`Self::set_anchors`] with ranges given in byte offsets into `source`,
    /// the markdown the document was read from. A range with an end the
    /// document has no place for comes in detached.
    pub fn set_source_anchors(
        &mut self,
        source: &str,
        anchors: Vec<(AnchorId, Range<usize>, Annotation)>,
        cx: &mut Context<Self>,
    ) {
        let offsets: Vec<usize> = anchors
            .iter()
            .flat_map(|(_, range, _)| [range.start, range.end])
            .collect();
        let ends: Vec<Option<Cursor>> = match self.mode {
            Mode::Blocks => markdown::parse_at_many(source, &offsets, &self.marks).1,
            Mode::Source => offsets
                .iter()
                .map(|offset| {
                    (*offset <= source.len()).then(|| Cursor::new(0, Part::Code, *offset))
                })
                .collect(),
        };
        self.anchors = anchors
            .into_iter()
            .enumerate()
            .map(|(ix, (id, _, state))| {
                let range = match (ends[2 * ix], ends[2 * ix + 1]) {
                    (Some(start), Some(end)) => Selection::new(start, end),
                    (surviving, other) => Selection::at(surviving.or(other).unwrap_or_default()),
                };
                Anchor {
                    id,
                    range: range.clamp(&self.doc),
                    state,
                }
            })
            .collect();
        cx.notify();
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
