//! `compile/layoutsize`: the unit's width and height, as a step per band or
//! a size in pixels, and the signals that hold them.

use crate::compile::{
    Channel, Unit,
    scale::{Component, Range},
};
use d3::format::number::Number;
use serde_json::{Value as Json, json};
use vega::spec as vg;

/// `LayoutSize`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Size {
    Step,
    Pixels(f64),
}

/// The unit's width and height.
#[derive(Clone, Copy, Debug)]
pub struct Sizes {
    pub width: Size,
    pub height: Size,
}

/// `parseUnitLayoutSize`.
pub fn parse(unit: &Unit, scales: &[Component]) -> Sizes {
    let size = |channel: Channel, specified: &Option<Json>| -> Size {
        if let Some(n) = specified.as_ref().and_then(Json::as_f64) {
            return Size::Pixels(n);
        }
        if specified.as_ref().is_some_and(|s| s.get("step").is_some()) {
            return Size::Step;
        }
        let view = |name: &str| unit.config.get("view", name).and_then(Json::as_f64);
        match scales.iter().find(|s| s.channel == channel) {
            Some(scale) if scale.is_discrete() => Size::Step,
            Some(_) => Size::Pixels(
                view(channel.size())
                    .or_else(|| {
                        view(match channel {
                            Channel::X => "continuousWidth",
                            Channel::Y => "continuousHeight",
                        })
                    })
                    .unwrap_or(300.0),
            ),
            None => Size::Pixels(view("step").unwrap_or(20.0)),
        }
    };
    Sizes {
        width: size(Channel::X, &unit.spec.width),
        height: size(Channel::Y, &unit.spec.height),
    }
}

/// `assembleLayoutSignals`.
pub fn signals(_unit: &Unit, scales: &[Component], sizes: &Sizes) -> Vec<vg::Signal> {
    let mut signals = Vec::new();
    for (channel, size) in [(Channel::X, sizes.width), (Channel::Y, sizes.height)] {
        let name = channel.size();
        match size {
            Size::Pixels(n) => signals.push(vg::Signal {
                name: name.into(),
                value: Some(json!(n)),
                ..Default::default()
            }),
            Size::Step => {
                let Some(scale) = scales.iter().find(|s| s.channel == channel) else {
                    continue;
                };
                let Range::Step(step) = scale.range else {
                    continue;
                };
                signals.push(vg::Signal {
                    name: format!("{}_step", scale.name),
                    value: Some(json!(step)),
                    ..Default::default()
                });
                let inner = match scale.type_ {
                    "band" => scale.padding_inner.or(scale.padding),
                    _ => Some(1.0),
                };
                let outer = scale.padding_outer.or(scale.padding);
                let print =
                    |v: Option<f64>| v.map_or("undefined".into(), |v| Number(v).to_string());
                signals.push(vg::Signal {
                    name: name.into(),
                    update: Some(format!(
                        "bandspace(domain('{s}').length, {}, {}) * {s}_step",
                        print(inner),
                        print(outer),
                        s = scale.name
                    )),
                    ..Default::default()
                });
            }
        }
    }
    signals
}
