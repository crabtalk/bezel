//! What a chart draws: layers over shared axes, each one [`Mark`] per row of
//! its [`Data`], placed by the fields its [`Encoding`] names.

use gpui::{Hsla, SharedString};

use crate::data::Data;

pub use channel::{Channel, Datum, Kind, Scale, Sort, Source};
pub use encoding::{Encode, Encoding, Position};
pub use mark::{
    Align, Baseline, Cap, Dir, Extent, Interpolate, Join, LineBreak, Mark, Orient, Overlay,
    Properties,
};
pub use shape::Shape;

mod channel;
mod encoding;
mod mark;
mod shape;

#[derive(Clone, Debug)]
pub struct Chart {
    /// Paint order. Their x scales are one scale, as are their y scales, and
    /// a colour value is one colour across them.
    pub layers: Vec<Layer>,
    pub title: Option<SharedString>,
}

#[derive(Clone, Debug)]
pub struct Layer {
    pub data: Data,
    pub mark: Mark,
    pub properties: Properties,
    pub encoding: Encoding,
}

impl Chart {
    /// One layer.
    pub fn new(mark: Mark, data: Data) -> Self {
        Self {
            layers: vec![Layer {
                data,
                mark,
                properties: Properties::default(),
                encoding: Encoding::default(),
            }],
            title: None,
        }
    }

    /// `other`'s layers over this chart's. Its title is dropped.
    pub fn layer(mut self, other: Chart) -> Self {
        self.layers.extend(other.layers);
        self
    }

    fn last(&mut self) -> &mut Encoding {
        &mut self
            .layers
            .last_mut()
            .expect("a chart has a layer")
            .encoding
    }

    /// Sets the last layer's mark properties.
    pub fn properties(mut self, properties: Properties) -> Self {
        self.layers
            .last_mut()
            .expect("a chart has a layer")
            .properties = properties;
        self
    }

    pub fn bar(data: Data) -> Self {
        Self::new(Mark::Bar, data)
    }

    pub fn line(data: Data) -> Self {
        Self::new(Mark::Line, data)
    }

    pub fn area(data: Data) -> Self {
        Self::new(Mark::Area, data)
    }

    pub fn point(data: Data) -> Self {
        Self::new(Mark::Point, data)
    }

    pub fn arc(data: Data) -> Self {
        Self::new(Mark::Arc, data)
    }

    /// Sets the last layer's x, as the other channels do theirs.
    pub fn x(mut self, x: impl Into<Encode<Position>>) -> Self {
        self.last().x = Some(x.into());
        self
    }

    pub fn y(mut self, y: impl Into<Encode<Position>>) -> Self {
        self.last().y = Some(y.into());
        self
    }

    pub fn x2(mut self, x2: impl Into<Encode<Position>>) -> Self {
        self.last().x2 = Some(x2.into());
        self
    }

    pub fn y2(mut self, y2: impl Into<Encode<Position>>) -> Self {
        self.last().y2 = Some(y2.into());
        self
    }

    pub fn x_offset(mut self, x_offset: impl Into<Encode<f32>>) -> Self {
        self.last().x_offset = Some(x_offset.into());
        self
    }

    pub fn y_offset(mut self, y_offset: impl Into<Encode<f32>>) -> Self {
        self.last().y_offset = Some(y_offset.into());
        self
    }

    pub fn theta(mut self, theta: impl Into<Encode<f32>>) -> Self {
        self.last().theta = Some(theta.into());
        self
    }

    pub fn theta2(mut self, theta2: impl Into<Encode<f32>>) -> Self {
        self.last().theta2 = Some(theta2.into());
        self
    }

    pub fn radius(mut self, radius: impl Into<Encode<f32>>) -> Self {
        self.last().radius = Some(radius.into());
        self
    }

    pub fn radius2(mut self, radius2: impl Into<Encode<f32>>) -> Self {
        self.last().radius2 = Some(radius2.into());
        self
    }

    pub fn color(mut self, color: impl Into<Encode<Hsla>>) -> Self {
        self.last().color = Some(color.into());
        self
    }

    pub fn fill(mut self, fill: impl Into<Encode<Hsla>>) -> Self {
        self.last().fill = Some(fill.into());
        self
    }

    pub fn stroke(mut self, stroke: impl Into<Encode<Hsla>>) -> Self {
        self.last().stroke = Some(stroke.into());
        self
    }

    pub fn opacity(mut self, opacity: impl Into<Encode<f32>>) -> Self {
        self.last().opacity = Some(opacity.into());
        self
    }

    pub fn fill_opacity(mut self, fill_opacity: impl Into<Encode<f32>>) -> Self {
        self.last().fill_opacity = Some(fill_opacity.into());
        self
    }

    pub fn stroke_opacity(mut self, stroke_opacity: impl Into<Encode<f32>>) -> Self {
        self.last().stroke_opacity = Some(stroke_opacity.into());
        self
    }

    pub fn size(mut self, size: impl Into<Encode<f32>>) -> Self {
        self.last().size = Some(size.into());
        self
    }

    pub fn stroke_width(mut self, stroke_width: impl Into<Encode<f32>>) -> Self {
        self.last().stroke_width = Some(stroke_width.into());
        self
    }

    pub fn stroke_dash(mut self, stroke_dash: impl Into<Encode<Vec<f32>>>) -> Self {
        self.last().stroke_dash = Some(stroke_dash.into());
        self
    }

    pub fn shape(mut self, shape: impl Into<Encode<Shape>>) -> Self {
        self.last().shape = Some(shape.into());
        self
    }

    pub fn angle(mut self, angle: impl Into<Encode<f32>>) -> Self {
        self.last().angle = Some(angle.into());
        self
    }

    pub fn text(mut self, text: impl Into<Encode<SharedString>>) -> Self {
        self.last().text = Some(text.into());
        self
    }

    pub fn href(mut self, href: impl Into<Encode<SharedString>>) -> Self {
        self.last().href = Some(href.into());
        self
    }

    pub fn url(mut self, url: impl Into<Encode<SharedString>>) -> Self {
        self.last().url = Some(url.into());
        self
    }

    /// Adds a detail channel to the last layer.
    pub fn detail(mut self, channel: Channel) -> Self {
        self.last().detail.push(channel);
        self
    }

    /// Adds an order channel to the last layer, after any it has.
    pub fn order(mut self, channel: Channel) -> Self {
        self.last().order.push(channel);
        self
    }

    pub fn key(mut self, channel: Channel) -> Self {
        self.last().key = Some(channel);
        self
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }
}
