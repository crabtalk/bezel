//! Vega-Lite's `compile`: a unit spec normalized, built into a model,
//! parsed into components, and assembled into a Vega spec.

use crate::{
    Error,
    config::Config,
    spec::{self, FieldDef, TopLevel, Type},
};
use serde_json::Value as Json;
use vega::spec as vg;

mod axis;
mod data;
mod field;
mod layout_size;
mod mark;
mod scale;

/// A position channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    X,
    Y,
}

impl Channel {
    pub fn name(self) -> &'static str {
        match self {
            Self::X => "x",
            Self::Y => "y",
        }
    }

    /// `getSizeChannel`.
    pub fn size(self) -> &'static str {
        match self {
            Self::X => "width",
            Self::Y => "height",
        }
    }

    pub fn other(self) -> Self {
        match self {
            Self::X => Self::Y,
            Self::Y => Self::X,
        }
    }
}

/// A mark's orientation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orient {
    Vertical,
    Horizontal,
}

/// `StackProperties`.
#[derive(Clone, Debug)]
pub struct Stack {
    pub field_channel: Channel,
    pub groupby_fields: Vec<String>,
    pub offset: vg::Offset,
}

/// A unit model: one mark, its encoding, and what Vega-Lite derives from
/// them.
pub struct Unit<'a> {
    pub spec: &'a TopLevel,
    pub config: &'a Config,
    /// The mark type.
    pub mark: String,
    pub orient: Option<Orient>,
    pub stack: Option<Stack>,
}

impl<'a> Unit<'a> {
    /// `UnitModel`'s constructor, after `normalize`.
    fn new(spec: &'a TopLevel, config: &'a Config) -> Result<Self, Error> {
        let mark = match &spec.mark {
            spec::Mark::Type(t) => t.clone(),
            spec::Mark::Def(def) => def.type_.clone(),
        };
        if !matches!(mark.as_str(), "bar" | "rect") {
            return Err(Error::Unsupported(format!("a {mark} mark")));
        }
        let mut unit = Self {
            spec,
            config,
            mark,
            orient: None,
            stack: None,
        };
        unit.orient = unit.default_orient();
        unit.stack = unit.default_stack();
        Ok(unit)
    }

    pub fn field_def(&self, channel: Channel) -> Option<&FieldDef> {
        match channel {
            Channel::X => self.spec.encoding.x.as_ref(),
            Channel::Y => self.spec.encoding.y.as_ref(),
        }
    }

    /// `getStyles`: the mark type, then its styles.
    pub fn styles(&self) -> Vec<String> {
        vec![self.mark.clone()]
    }

    /// `getMarkPropOrConfig`, for a mark definition that sets nothing.
    pub fn mark_config(&self, name: &str, vg_channel: Option<&str>) -> Option<&Json> {
        self.config
            .mark(name, &self.mark, &self.styles(), vg_channel)
    }

    /// `init.ts`'s `orient`, for a bar or rect.
    fn default_orient(&self) -> Option<Orient> {
        if self.mark == "rect" {
            return None;
        }
        let measure = |def: Option<&FieldDef>| def.is_some_and(|d| d.type_ == Type::Quantitative);
        let (x, y) = (self.field_def(Channel::X), self.field_def(Channel::Y));
        match (measure(x), measure(y)) {
            (true, false) => Some(Orient::Horizontal),
            (false, true) | (true, true) => Some(Orient::Vertical),
            (false, false) => {
                let temporal =
                    |def: Option<&FieldDef>| def.is_some_and(|d| d.type_ == Type::Temporal);
                match (temporal(x), temporal(y)) {
                    (true, false) => Some(Orient::Vertical),
                    (false, true) => Some(Orient::Horizontal),
                    _ => Some(Orient::Vertical),
                }
            }
        }
    }

    /// `stack.ts`'s `stack`: a bar stacks its measure by default.
    fn default_stack(&self) -> Option<Stack> {
        if self.mark != "bar" {
            return None;
        }
        let channel = self.potential_stacked_channel()?;
        let stacked = field::vg_field(self.field_def(channel)?, &field::Options::default());
        let mut groupby_fields = Vec::new();
        if let Some(dimension) = self.field_def(channel.other()) {
            let name = field::vg_field(dimension, &field::Options::default());
            if name != stacked {
                groupby_fields.push(name);
            }
        }
        Some(Stack {
            field_channel: channel,
            groupby_fields,
            offset: vg::Offset::Zero,
        })
    }

    /// `potentialStackedChannel` for `x` and `y`.
    fn potential_stacked_channel(&self) -> Option<Channel> {
        let quantitative =
            |def: Option<&FieldDef>| def.is_some_and(|d| d.type_ == Type::Quantitative);
        let (x, y) = (self.field_def(Channel::X), self.field_def(Channel::Y));
        let bar = self.mark == "bar";
        if x.is_some() && y.is_some() {
            match (quantitative(x), quantitative(y)) {
                (true, true) => match self.orient {
                    Some(Orient::Vertical) if bar => Some(Channel::Y),
                    Some(Orient::Horizontal) if bar => Some(Channel::X),
                    _ => None,
                },
                (true, false) => Some(Channel::X),
                (false, true) => Some(Channel::Y),
                (false, false) => None,
            }
        } else if quantitative(x) {
            (!(bar && self.orient == Some(Orient::Vertical))).then_some(Channel::X)
        } else if quantitative(y) {
            (!(bar && self.orient == Some(Orient::Horizontal))).then_some(Channel::Y)
        } else {
            None
        }
    }

    /// The scales the unit has, by channel.
    pub fn scale_channels(&self) -> Vec<Channel> {
        [Channel::X, Channel::Y]
            .into_iter()
            .filter(|c| self.field_def(*c).is_some())
            .collect()
    }
}

/// JavaScript's `!!value` on a JSON value.
pub fn truthy(value: &Json) -> bool {
    match value {
        Json::Null => false,
        Json::Bool(b) => *b,
        Json::Number(n) => n.as_f64().is_some_and(|n| n != 0.0 && !n.is_nan()),
        Json::String(s) => !s.is_empty(),
        _ => true,
    }
}

/// The Vega spec `spec` compiles to.
pub fn compile(spec: &TopLevel) -> Result<vg::Spec, Error> {
    let config = Config::default();
    let unit = Unit::new(spec, &config)?;
    let scales = scale::parse(&unit)?;
    let sizes = layout_size::parse(&unit, &scales);
    let data = data::parse(&unit, &scales)?;
    let axes = axis::parse(&unit, &scales, &sizes)?;
    let marks = mark::parse(&unit, &scales, &sizes, &data.main)?;
    let mut signals = layout_size::signals(&unit, &scales, &sizes);
    let mut out = vg::Spec {
        schema: Some("https://vega.github.io/schema/vega/v6.json".into()),
        description: spec.description.clone(),
        background: config.0.get("background").cloned(),
        padding: config.0.get("padding").cloned(),
        style: Some(vg::Many::One(
            if spec.encoding.x.is_some() || spec.encoding.y.is_some() {
                "cell"
            } else {
                "view"
            }
            .into(),
        )),
        data: data.assembled.clone(),
        marks,
        scales: scale::assemble(&scales, &data.main),
        axes,
        ..vg::Spec::default()
    };
    // A size signal with a value becomes the spec's own width or height.
    signals.retain(|signal| match (signal.name.as_str(), &signal.value) {
        ("width", Some(value)) => {
            out.width = Some(value.clone());
            false
        }
        ("height", Some(value)) => {
            out.height = Some(value.clone());
            false
        }
        _ => true,
    });
    out.signals = signals;
    Ok(out)
}
