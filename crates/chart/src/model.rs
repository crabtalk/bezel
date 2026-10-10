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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Encoding {
    pub x: Option<Channel>,
    pub y: Option<Channel>,
    /// One series per value.
    pub color: Option<Channel>,
    pub theta: Option<Channel>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Channel {
    pub field: SharedString,
    pub kind: Kind,
    /// The axis's or legend's title. `None` shows none.
    pub title: Option<SharedString>,
    pub scale: Scale,
    /// The order of a discrete field's values.
    pub sort: Sort,
}

/// How a continuous field's values map to positions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scale {
    /// Whether a quantitative scale takes in zero. A stack of bars always
    /// starts at zero.
    pub zero: bool,
    /// Fixed `[low, high]`, times in milliseconds since the epoch. Marks
    /// outside it are cut off at the plot's edge.
    pub domain: Option<[f64; 2]>,
    /// Whether a scale with no fixed domain widens to the ticks either side.
    pub nice: bool,
}

impl Default for Scale {
    fn default() -> Self {
        Self {
            zero: true,
            domain: None,
            nice: true,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Sort {
    /// The order rows first show each value in.
    #[default]
    Data,
    Ascending,
    Descending,
    /// These values first, in this order, then the rest in data order. A
    /// number matches as its read-out text.
    Explicit(Vec<SharedString>),
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

    pub fn x(mut self, channel: Channel) -> Self {
        self.encoding.x = Some(channel);
        self
    }

    pub fn y(mut self, channel: Channel) -> Self {
        self.encoding.y = Some(channel);
        self
    }

    pub fn color(mut self, channel: Channel) -> Self {
        self.encoding.color = Some(channel);
        self
    }

    pub fn theta(mut self, channel: Channel) -> Self {
        self.encoding.theta = Some(channel);
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
            title: None,
            scale: Scale::default(),
            sort: Sort::default(),
        }
    }

    pub fn quantitative(field: impl Into<SharedString>) -> Self {
        Self::new(field, Kind::Quantitative)
    }

    pub fn temporal(field: impl Into<SharedString>) -> Self {
        Self::new(field, Kind::Temporal)
    }

    pub fn ordinal(field: impl Into<SharedString>) -> Self {
        Self::new(field, Kind::Ordinal)
    }

    pub fn nominal(field: impl Into<SharedString>) -> Self {
        Self::new(field, Kind::Nominal)
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn zero(mut self, zero: bool) -> Self {
        self.scale.zero = zero;
        self
    }

    pub fn domain(mut self, low: f64, high: f64) -> Self {
        self.scale.domain = Some([low, high]);
        self
    }

    pub fn nice(mut self, nice: bool) -> Self {
        self.scale.nice = nice;
        self
    }

    pub fn sort(mut self, sort: Sort) -> Self {
        self.sort = sort;
        self
    }
}
