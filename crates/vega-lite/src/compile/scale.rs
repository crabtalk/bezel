//! `compile/scale`: each channel's scale type, domain, range and
//! properties, and the Vega scales they assemble to.

use crate::{
    Error,
    compile::{Channel, Orient, Unit, field},
    spec::Type,
};
use serde_json::{Value as Json, json};
use vega::spec as vg;

/// A scale's domain before data names are resolved.
#[derive(Clone, Debug)]
pub enum Domain {
    /// The values of `field` in the main data, sorted where `sort` is set.
    Field { field: String, sort: bool },
    /// The extent of each of `fields` in the main data.
    Fields(Vec<String>),
}

/// A scale's range.
#[derive(Clone, Debug)]
pub enum Range {
    /// A band or point scale `step` pixels a band.
    Step(f64),
    /// The view's width or height.
    Size(&'static str),
}

/// `ScaleComponent`.
#[derive(Clone, Debug)]
pub struct Component {
    pub channel: Channel,
    pub name: String,
    pub type_: &'static str,
    pub domain: Domain,
    pub range: Range,
    pub padding: Option<f64>,
    pub padding_inner: Option<f64>,
    pub padding_outer: Option<f64>,
    pub nice: Option<bool>,
    pub zero: Option<bool>,
}

impl Component {
    /// `hasDiscreteDomain`.
    pub fn is_discrete(&self) -> bool {
        matches!(self.type_, "band" | "point")
    }
}

fn number(value: Option<&Json>) -> Option<f64> {
    value.and_then(Json::as_f64)
}

/// `scale/type.ts`'s `defaultType` for a position channel.
fn default_type(unit: &Unit, type_: Type) -> Result<&'static str, Error> {
    Ok(match type_ {
        Type::Nominal | Type::Ordinal => match unit.mark.as_str() {
            "rect" | "bar" | "image" | "rule" | "tick" => "band",
            _ => "point",
        },
        Type::Quantitative => "linear",
        Type::Temporal => return Err(Error::Unsupported("a temporal field".into())),
    })
}

/// `parseScales` for a unit.
pub fn parse(unit: &Unit) -> Result<Vec<Component>, Error> {
    let config = unit.config;
    let mut scales = Vec::new();
    for channel in unit.scale_channels() {
        let def = unit
            .field_def(channel)
            .expect("a scale channel has a field");
        let type_ = default_type(unit, def.type_)?;
        let continuous = type_ == "linear";
        // `padding`.
        let padding = if continuous {
            let vertical_x = unit.orient == Some(Orient::Vertical) && channel == Channel::X;
            let horizontal_y = unit.orient == Some(Orient::Horizontal) && channel == Channel::Y;
            match unit.mark.as_str() {
                "bar" if vertical_x || horizontal_y => {
                    number(config.get("bar", "continuousBandSize"))
                }
                _ => None,
            }
        } else if type_ == "point" {
            number(config.get("scale", "pointPadding"))
        } else {
            None
        };
        // `paddingInner`, for a band scale.
        let padding_inner = (type_ == "band" && padding.is_none())
            .then(|| {
                number(config.get("scale", "bandPaddingInner")).or_else(|| {
                    number(config.get(
                        "scale",
                        match unit.mark.as_str() {
                            "bar" => "barBandPaddingInner",
                            "tick" => "tickBandPaddingInner",
                            _ => "rectBandPaddingInner",
                        },
                    ))
                })
            })
            .flatten();
        // `paddingOuter`.
        let padding_outer = (type_ == "band" && padding.is_none())
            .then(|| {
                number(config.get("scale", "bandPaddingOuter")).or(padding_inner.map(|p| p / 2.0))
            })
            .flatten();
        // `nice` and `zero`.
        let nice = continuous.then_some(true);
        let zero = continuous.then(|| {
            let along = match channel {
                Channel::X => unit.orient == Some(Orient::Vertical),
                Channel::Y => unit.orient == Some(Orient::Horizontal),
            };
            if matches!(unit.mark.as_str(), "bar" | "area" | "line" | "trail") && along {
                false
            } else if matches!(unit.mark.as_str(), "bar" | "area") {
                true
            } else {
                config
                    .get("scale", "zero")
                    .and_then(Json::as_bool)
                    .unwrap_or(false)
            }
        });
        let domain = match &unit.stack {
            Some(stack) if stack.field_channel == channel => Domain::Fields(vec![
                field::vg_field(
                    def,
                    &field::Options {
                        suffix: Some("start"),
                        ..Default::default()
                    },
                ),
                field::vg_field(
                    def,
                    &field::Options {
                        suffix: Some("end"),
                        ..Default::default()
                    },
                ),
            ]),
            _ => Domain::Field {
                field: field::vg_field(def, &field::Options::default()),
                sort: !continuous,
            },
        };
        let range = if continuous {
            Range::Size(channel.size())
        } else {
            Range::Step(number(config.get("view", "step")).unwrap_or(20.0))
        };
        scales.push(Component {
            channel,
            name: channel.name().to_string(),
            type_,
            domain,
            range,
            padding,
            padding_inner,
            padding_outer,
            nice,
            zero,
        });
    }
    Ok(scales)
}

/// `assembleScales`, the main data named `main`.
pub fn assemble(scales: &[Component], main: &str) -> Vec<vg::Scale> {
    scales
        .iter()
        .map(|scale| vg::Scale {
            name: scale.name.clone(),
            type_: Some(scale.type_.to_string()),
            domain: Some(match &scale.domain {
                Domain::Field { field, sort } => vg::Domain::Data(vg::DataDomain {
                    data: Some(main.into()),
                    field: Some(field.clone()),
                    sort: sort.then_some(vg::Sort::Flag(true)),
                    ..Default::default()
                }),
                Domain::Fields(fields) => vg::Domain::Data(vg::DataDomain {
                    data: Some(main.into()),
                    fields: Some(fields.iter().cloned().map(vg::DomainField::Name).collect()),
                    ..Default::default()
                }),
            }),
            range: Some(match scale.range {
                Range::Step(_) => vg::Range::Step(vg::StepRange {
                    step: json!({"signal": format!("{}_step", scale.name)}),
                }),
                Range::Size("width") => {
                    vg::Range::Values(vec![json!(0), json!({"signal": "width"})])
                }
                Range::Size(_) => vg::Range::Values(vec![json!({"signal": "height"}), json!(0)]),
            }),
            padding: scale.padding.map(Json::from),
            padding_inner: scale.padding_inner.map(Json::from),
            padding_outer: scale.padding_outer.map(Json::from),
            nice: scale.nice.map(vg::Nice::Flag),
            zero: scale.zero.map(Json::from),
            ..Default::default()
        })
        .collect()
}
