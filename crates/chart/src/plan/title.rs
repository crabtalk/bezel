//! The chart's title and subtitle, as Vega stands them over or under
//! everything else.

use crate::{
    model::{Align, Anchor, Baseline, Frame, Side, Title},
    plan::{Ink, Label, Rect, layout, layout::Layout},
};

/// Vega's title defaults, in pixels.
const OFFSET: f32 = 4.0;
const SUBTITLE_PADDING: f32 = 3.0;

/// A title measured, its title's top-left and its subtitle's at the origin.
pub(super) struct Heading {
    title: Label,
    subtitle: Option<Label>,
    side: Side,
    anchor: Anchor,
    frame: Frame,
    offset: f32,
    align: Option<Align>,
    shift: [f32; 2],
    over: bool,
}

impl Heading {
    pub(super) fn of(title: &Title, layout: &mut Layout) -> Self {
        let label = layout.label(
            &title.text,
            &title.lettering,
            (layout::CHART_TITLE, true),
            Ink::Text,
            0.0,
        );
        let below = label.height();
        let subtitle = title.subtitle.as_ref().map(|text| {
            let label = layout.label(
                text,
                &title.subtitle_lettering,
                (layout::SUBTITLE, false),
                Ink::Muted,
                0.0,
            );
            let padding = title.subtitle_padding.unwrap_or(SUBTITLE_PADDING);
            label.moved([0.0, below + padding])
        });
        Self {
            title: label,
            subtitle,
            side: title.orient.unwrap_or(Side::Top),
            anchor: title.anchor.unwrap_or(Anchor::Middle),
            frame: title.frame.unwrap_or(Frame::Group),
            offset: title.offset.unwrap_or(OFFSET),
            align: title.lettering.align,
            shift: [title.dx, title.dy],
            over: title.zindex.is_some_and(|z| z > 0.0),
        }
    }

    pub(super) fn side(&self) -> Side {
        self.side
    }

    /// Whether it is drawn over the marks rather than under them.
    pub(super) fn over(&self) -> bool {
        self.over
    }

    /// The room it takes from its side.
    pub(super) fn extent(&self) -> f32 {
        let block = self.block();
        let across = match self.side.is_horizontal() {
            true => block.h,
            false => block.w,
        };
        across + self.offset
    }

    /// Its title and subtitle together.
    fn block(&self) -> Rect {
        let title = self.title.rect();
        self.subtitle
            .as_ref()
            .map_or(title, |subtitle| title.union(subtitle.rect()))
    }

    /// Its title and subtitle beside `bounds`, what else is drawn,
    /// anchored along the plot or along `bounds` as its frame says: over
    /// or under them from the left, or beside them from the bottom on the
    /// left and from the top on the right.
    pub(super) fn place(self, plot: Rect, bounds: Rect) -> (Label, Option<Label>) {
        let frame = match self.frame {
            Frame::Group => plot,
            Frame::Bounds => bounds,
        };
        let block = self.block();
        let fraction = self.anchor.fraction();
        let align = self.align.unwrap_or(match self.anchor {
            Anchor::Start => Align::Left,
            Anchor::Middle => Align::Center,
            Anchor::End => Align::Right,
        });
        // Where the block's anchor and top stand.
        let (x, y, align) = match self.side {
            Side::Top | Side::Bottom => {
                let x = frame.x + frame.w * fraction;
                let y = match self.side {
                    Side::Bottom => bounds.bottom() + self.offset,
                    _ => bounds.y - block.h - self.offset,
                };
                (x, y, align)
            }
            Side::Left | Side::Right => {
                let along = match self.side {
                    Side::Left => frame.bottom() - frame.h * fraction,
                    _ => frame.y + frame.h * fraction,
                };
                let y = along - block.h * fraction;
                let x = match self.side {
                    Side::Left => bounds.x - block.w - self.offset,
                    _ => bounds.right() + self.offset,
                };
                (x + block.w * align.fraction(), y, align)
            }
        };
        let [dx, dy] = self.shift;
        let place = |label: Label| {
            let top = label.at[1];
            label.anchored([x + dx, y + top + dy], align, Baseline::Top)
        };
        (place(self.title), self.subtitle.map(place))
    }
}
