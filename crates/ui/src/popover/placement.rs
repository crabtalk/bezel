use gpui::{Anchor, Pixels, Point, Size, point};

pub struct Placement {
    pub available_height: Pixels,
    at: Point<Pixels>,
    anchor: Anchor,
    above: bool,
    viewport: Size<Pixels>,
    margin: Pixels,
    gap: Pixels,
}

impl Placement {
    pub fn new(
        anchor: Anchor,
        at: Point<Pixels>,
        height: Pixels,
        viewport: Size<Pixels>,
        margin: Pixels,
        gap: Pixels,
    ) -> Self {
        let maximum = (viewport.height - margin * 2.).max(Pixels::ZERO);
        let above = (at.y - margin - gap).clamp(Pixels::ZERO, maximum);
        let below = (viewport.height - margin - at.y - gap).clamp(Pixels::ZERO, maximum);
        let prefer_above = matches!(anchor, Anchor::BottomLeft | Anchor::BottomRight);
        let (preferred, other) = if prefer_above {
            (above, below)
        } else {
            (below, above)
        };
        let flip = height > preferred && other > preferred;
        Self {
            available_height: if flip { other } else { preferred },
            at,
            anchor,
            above: prefer_above != flip,
            viewport,
            margin,
            gap,
        }
    }

    pub fn origin(&self, size: Size<Pixels>) -> Point<Pixels> {
        let x = if matches!(self.anchor, Anchor::TopRight | Anchor::BottomRight) {
            self.at.x - size.width
        } else {
            self.at.x
        };
        let y = if self.above {
            self.at.y - self.gap - size.height
        } else {
            self.at.y + self.gap
        };
        point(
            x.clamp(
                self.margin,
                (self.viewport.width - self.margin - size.width).max(self.margin),
            ),
            y.clamp(
                self.margin,
                (self.viewport.height - self.margin - size.height).max(self.margin),
            ),
        )
    }
}
