//! What a chart draws: one [`Mark`] per row of its [`Data`], placed by the
//! fields its [`Encoding`] names.

use gpui::SharedString;

use crate::data::Data;

#[derive(Clone, Debug)]
pub struct Chart {
    pub data: Data,
    pub mark: Mark,
    pub encoding: Encoding,
    pub title: Option<SharedString>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mark {
    Bar,
    Line,
    Area,
    Point,
    /// Pie and donut slices, sized by `theta`.
    Arc,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Encoding {
    pub x: Option<Channel>,
    pub y: Option<Channel>,
    /// One series per value.
    pub color: Option<Channel>,
    pub theta: Option<Channel>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Channel {
    pub field: SharedString,
    pub kind: Kind,
}

/// How a field's values are read, whatever column holds them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Quantitative,
    Temporal,
    Ordinal,
    Nominal,
}

impl Chart {
    pub fn new(mark: Mark, data: Data) -> Self {
        Self {
            data,
            mark,
            encoding: Encoding::default(),
            title: None,
        }
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

    pub fn x(mut self, field: impl Into<SharedString>, kind: Kind) -> Self {
        self.encoding.x = Some(Channel::new(field, kind));
        self
    }

    pub fn y(mut self, field: impl Into<SharedString>, kind: Kind) -> Self {
        self.encoding.y = Some(Channel::new(field, kind));
        self
    }

    pub fn color(mut self, field: impl Into<SharedString>, kind: Kind) -> Self {
        self.encoding.color = Some(Channel::new(field, kind));
        self
    }

    pub fn theta(mut self, field: impl Into<SharedString>, kind: Kind) -> Self {
        self.encoding.theta = Some(Channel::new(field, kind));
        self
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }
}

impl Channel {
    pub fn new(field: impl Into<SharedString>, kind: Kind) -> Self {
        Self {
            field: field.into(),
            kind,
        }
    }
}
