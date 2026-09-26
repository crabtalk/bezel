//! The document's column, built a block at a time as far as the window shows.

use super::*;
use gpui::{
    AvailableSpace, GlobalElementId, InspectorElementId, LayoutId, ScrollHandle, Size, Style,
    relative,
};

/// Builds one block's box, without the gap above it.
pub(super) type BuildBlock = Box<dyn Fn(usize, &mut Window, &mut App) -> AnyElement>;

/// What a block is guessed at before it has ever been measured.
#[derive(Clone, Copy)]
pub(super) struct Guess {
    /// Characters set in wrapping text, and the line they are set on.
    pub(super) chars: usize,
    pub(super) line: Pixels,
    /// Rows that do not wrap (a fence's lines, a table's rows).
    pub(super) rows: usize,
    /// Height that is not text: padding, a picture, a card.
    pub(super) extra: Pixels,
    pub(super) indent: Pixels,
}

impl Guess {
    fn height(self, width: Pixels) -> Pixels {
        let room = (width - self.indent).max(px(1.0));
        // Roughly a proportional face's average advance.
        let advance = self.line * 0.36;
        let wrapped = ((advance * self.chars as f32) / room).ceil() as usize;
        self.extra + self.line * (wrapped + self.rows).max(1) as f32
    }
}

/// The column [`render_with`] hands back when it has [`BlockLayouts`] to
/// measure into.
///
/// Its height is every block's last measured height, at whatever width that
/// was, or its [`Guess`], plus the gaps. At prepaint it builds the blocks within half a screen of the part
/// of the window it shows, and those in `keep`, and lays them out around the
/// first block showing, which stays where the last frame's heights put it.
/// Heights that come out different above that block move `scroll` by the
/// difference, so the next frame shows the same text in the same place.
pub(super) struct Column {
    pub(super) layouts: BlockLayouts,
    pub(super) keys: Rc<[u64]>,
    pub(super) gaps: Rc<[Pixels]>,
    pub(super) guesses: Rc<[Guess]>,
    pub(super) keep: Vec<usize>,
    pub(super) scroll: Option<ScrollHandle>,
    pub(super) build: BuildBlock,
}

impl Column {
    fn heights(&self, width: Pixels) -> Vec<Pixels> {
        heights(&self.layouts, &self.keys, &self.guesses, width)
    }
}

fn heights(layouts: &BlockLayouts, keys: &[u64], guesses: &[Guess], width: Pixels) -> Vec<Pixels> {
    keys.iter()
        .zip(guesses)
        .map(|(key, guess)| layouts.height(*key).unwrap_or_else(|| guess.height(width)))
        .collect()
}

pub(super) struct Built {
    ix: usize,
    element: AnyElement,
}

impl IntoElement for Column {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Column {
    type RequestLayoutState = ();
    type PrepaintState = Vec<Built>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        _: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        let (layouts, keys, gaps, guesses) = (
            self.layouts.clone(),
            self.keys.clone(),
            self.gaps.clone(),
            self.guesses.clone(),
        );
        let layout = window.request_measured_layout(style, move |known, available, _, _| {
            let width = known.width.unwrap_or(match available.width {
                AvailableSpace::Definite(width) => width,
                AvailableSpace::MinContent | AvailableSpace::MaxContent => px(0.0),
            });
            let blocks: Pixels = heights(&layouts, &keys, &guesses, width).into_iter().sum();
            size(width, blocks + gaps.iter().copied().sum::<Pixels>())
        });
        (layout, ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<Built> {
        // Emptied here rather than at build: an editor reads last frame's
        // positions while it builds this frame's tree.
        self.layouts.clear();
        let width = bounds.size.width;
        let count = self.keys.len();
        if count == 0 {
            return Vec::new();
        }
        let heights = self.heights(width);
        let mut tops = Vec::with_capacity(count);
        let mut y = px(0.0);
        for (gap, height) in self.gaps.iter().zip(&heights) {
            y += *gap;
            tops.push(y);
            y += *height;
        }
        let bottom = |ix: usize| tops[ix] + heights[ix];

        let shown = window.content_mask().bounds.intersect(&bounds);
        let (from, to) = (shown.top() - bounds.top(), shown.bottom() - bounds.top());
        let margin = (to - from).max(px(0.0)) * 0.5;
        let first = tops.partition_point(|top| *top < from - margin);
        let first = first.saturating_sub(1);
        let mut last = first;
        while last + 1 < count && tops[last + 1] <= to + margin {
            last += 1;
        }
        let first = (first..=last)
            .find(|ix| bottom(*ix) >= from - margin)
            .unwrap_or(last);
        let anchor = (first..=last)
            .find(|ix| bottom(*ix) > from)
            .unwrap_or(first);

        let mut built: Vec<usize> = (first..=last).collect();
        built.extend(self.keep.iter().copied().filter(|ix| *ix < count));
        built.sort_unstable();
        built.dedup();

        let space = Size {
            width: AvailableSpace::Definite(width),
            height: AvailableSpace::MinContent,
        };
        let mut items: Vec<(Built, Pixels)> = built
            .into_iter()
            .map(|ix| {
                let mut element = (self.build)(ix, window, cx);
                let measured = element.layout_as_root(space, window, cx).height;
                (Built { ix, element }, measured)
            })
            .collect();

        // Laid out from the anchor both ways with what was measured, so the
        // text showing stays where last frame's heights put it.
        let mut placed = vec![None; count];
        let mut shift = px(0.0);
        let mut changed = false;
        for (item, measured) in &items {
            let grew = *measured - heights[item.ix];
            if grew.abs() > px(0.5) {
                changed = true;
                if item.ix < anchor {
                    shift += grew;
                }
            }
        }
        let measured_at = |ix: usize| {
            items
                .iter()
                .find(|(item, _)| item.ix == ix)
                .map(|(_, measured)| *measured)
        };
        let mut y = tops[anchor];
        for ix in anchor..=last {
            placed[ix] = Some(y);
            y += measured_at(ix).unwrap_or(heights[ix]);
            if ix + 1 < count {
                y += self.gaps[ix + 1];
            }
        }
        let mut y = tops[anchor];
        for ix in (first..anchor).rev() {
            y -= self.gaps[ix + 1] + measured_at(ix).unwrap_or(heights[ix]);
            placed[ix] = Some(y);
        }

        for (item, measured) in &mut items {
            let top = placed[item.ix].unwrap_or(tops[item.ix]);
            item.element
                .prepaint_at(bounds.origin + point(px(0.0), top), window, cx);
            self.layouts.record_height(self.keys[item.ix], *measured);
        }

        if shift != px(0.0)
            && let Some(scroll) = &self.scroll
        {
            let offset = scroll.offset();
            scroll.set_offset(point(offset.x, offset.y - shift));
        }
        if changed {
            window.request_animation_frame();
        }
        items.into_iter().map(|(item, _)| item).collect()
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        built: &mut Vec<Built>,
        window: &mut Window,
        cx: &mut App,
    ) {
        for item in built {
            item.element.paint(window, cx);
        }
    }
}
