//! Which fields, datums and values a layer's marks are drawn from, by
//! Vega-Lite channel.

use gpui::{Hsla, SharedString};

use crate::model::{Channel, Shape};

/// A channel's data through its scale, or one value in its own range.
#[derive(Clone, Debug, PartialEq)]
pub enum Encode<V> {
    Scaled(Channel),
    Value(V),
}

impl<V> From<Channel> for Encode<V> {
    fn from(channel: Channel) -> Self {
        Self::Scaled(channel)
    }
}

impl<V> Encode<V> {
    pub fn channel(&self) -> Option<&Channel> {
        match self {
            Self::Scaled(channel) => Some(channel),
            Self::Value(_) => None,
        }
    }
}

/// A position channel's value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Position {
    /// From the plot's left edge for x and x2, its top edge for y and y2.
    Pixels(f32),
    /// The plot's width for x and x2, its height for y and y2: Vega-Lite's
    /// `"width"` and `"height"`.
    Size,
}

impl Position {
    /// Pixels from the plot's edge, along an axis `size` long.
    pub fn pixels(self, size: f32) -> f32 {
        match self {
            Position::Pixels(pixels) => pixels,
            Position::Size => size,
        }
    }
}

/// Values are in pixels, square pixels for a point's `size`, degrees for
/// `angle` and radians for `theta`; a position's is a [`Position`]. `x2`, `y2`, `theta2` and `radius2` share
/// their primary channel's scale.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Encoding {
    pub x: Option<Encode<Position>>,
    pub y: Option<Encode<Position>>,
    /// Where a ranged bar or area ends along x.
    pub x2: Option<Encode<Position>>,
    pub y2: Option<Encode<Position>>,
    /// A place within x's band.
    pub x_offset: Option<Encode<f32>>,
    pub y_offset: Option<Encode<f32>>,
    pub theta: Option<Encode<f32>>,
    /// Where a slice ends, in place of stacking.
    pub theta2: Option<Encode<f32>>,
    /// A slice's outer radius.
    pub radius: Option<Encode<f32>>,
    /// A slice's inner radius.
    pub radius2: Option<Encode<f32>>,
    /// One series per value.
    pub color: Option<Encode<Hsla>>,
    /// Over `color`, for a mark's inside.
    pub fill: Option<Encode<Hsla>>,
    /// Over `color`, for a mark's outline or a line.
    pub stroke: Option<Encode<Hsla>>,
    pub opacity: Option<Encode<f32>>,
    pub fill_opacity: Option<Encode<f32>>,
    pub stroke_opacity: Option<Encode<f32>>,
    /// A point's area, a line's, rule's or trail's width, a bar's or tick's
    /// thickness, a text's font size.
    pub size: Option<Encode<f32>>,
    pub stroke_width: Option<Encode<f32>>,
    /// Lengths of dash and gap, in turn.
    pub stroke_dash: Option<Encode<Vec<f32>>>,
    pub shape: Option<Encode<Shape>>,
    /// Clockwise.
    pub angle: Option<Encode<f32>>,
    pub text: Option<Encode<SharedString>>,
    /// The url a click on the mark reports.
    pub href: Option<Encode<SharedString>>,
    /// An image's url, as the spec writes it.
    pub url: Option<Encode<SharedString>>,
    /// Each splits series as `color` does, with no colour of its own.
    pub detail: Vec<Channel>,
    /// The order rows are joined, stacked and drawn in, by the first channel
    /// then the next.
    pub order: Vec<Channel>,
    /// Each row's identity. Splits series as `detail` does.
    pub key: Option<Channel>,
}

impl Encoding {
    /// Every channel that reads a field or datum, primaries before the
    /// channels that share their scales.
    pub fn channels(&self) -> Vec<&Channel> {
        let positions = [&self.x, &self.y];
        let numbers = [
            &self.x_offset,
            &self.y_offset,
            &self.theta,
            &self.radius,
            &self.opacity,
            &self.fill_opacity,
            &self.stroke_opacity,
            &self.size,
            &self.stroke_width,
            &self.angle,
        ];
        let secondaries = [&self.x2, &self.y2];
        let numbers2 = [&self.theta2, &self.radius2];
        let colors = [&self.color, &self.fill, &self.stroke];
        let texts = [&self.text, &self.href, &self.url];
        positions
            .into_iter()
            .filter_map(|e| e.as_ref()?.channel())
            .chain(numbers.into_iter().filter_map(|e| e.as_ref()?.channel()))
            .chain(
                secondaries
                    .into_iter()
                    .filter_map(|e| e.as_ref()?.channel()),
            )
            .chain(numbers2.into_iter().filter_map(|e| e.as_ref()?.channel()))
            .chain(colors.into_iter().filter_map(|e| e.as_ref()?.channel()))
            .chain(self.stroke_dash.as_ref().and_then(Encode::channel))
            .chain(self.shape.as_ref().and_then(Encode::channel))
            .chain(texts.into_iter().filter_map(|e| e.as_ref()?.channel()))
            .chain(&self.detail)
            .chain(&self.order)
            .chain(&self.key)
            .collect()
    }

    /// Its channels over `under`'s: each it has, and `under`'s where it has
    /// none. `detail`, `order` and `key` are `under`'s.
    pub fn over(&self, under: &Encoding) -> Encoding {
        Encoding {
            x: self.x.clone().or_else(|| under.x.clone()),
            y: self.y.clone().or_else(|| under.y.clone()),
            x2: self.x2.clone().or_else(|| under.x2.clone()),
            y2: self.y2.clone().or_else(|| under.y2.clone()),
            x_offset: self.x_offset.clone().or_else(|| under.x_offset.clone()),
            y_offset: self.y_offset.clone().or_else(|| under.y_offset.clone()),
            theta: self.theta.clone().or_else(|| under.theta.clone()),
            theta2: self.theta2.clone().or_else(|| under.theta2.clone()),
            radius: self.radius.clone().or_else(|| under.radius.clone()),
            radius2: self.radius2.clone().or_else(|| under.radius2.clone()),
            color: self.color.clone().or_else(|| under.color.clone()),
            fill: self.fill.clone().or_else(|| under.fill.clone()),
            stroke: self.stroke.clone().or_else(|| under.stroke.clone()),
            opacity: self.opacity.clone().or_else(|| under.opacity.clone()),
            fill_opacity: self
                .fill_opacity
                .clone()
                .or_else(|| under.fill_opacity.clone()),
            stroke_opacity: self
                .stroke_opacity
                .clone()
                .or_else(|| under.stroke_opacity.clone()),
            size: self.size.clone().or_else(|| under.size.clone()),
            stroke_width: self
                .stroke_width
                .clone()
                .or_else(|| under.stroke_width.clone()),
            stroke_dash: self
                .stroke_dash
                .clone()
                .or_else(|| under.stroke_dash.clone()),
            shape: self.shape.clone().or_else(|| under.shape.clone()),
            angle: self.angle.clone().or_else(|| under.angle.clone()),
            text: self.text.clone().or_else(|| under.text.clone()),
            href: self.href.clone().or_else(|| under.href.clone()),
            url: self.url.clone().or_else(|| under.url.clone()),
            detail: under.detail.clone(),
            order: under.order.clone(),
            key: under.key.clone(),
        }
    }
}
