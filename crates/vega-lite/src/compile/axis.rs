//! `compile/axis`: each position channel's axis, its properties from
//! Vega-Lite's rules, assembled to a grid axis and a main axis.

use crate::{
    Error,
    compile::{Channel, Unit, field, layout_size::Sizes, scale::Component},
    spec::{Axis, Type},
};
use serde_json::{Value as Json, json};
use vega::spec as vg;

/// `normalizeAngle`.
fn normalize_angle(angle: f64) -> f64 {
    ((angle % 360.0) + 360.0) % 360.0
}

/// `getLabelAngle`.
fn label_angle(axis: &Axis, channel: Channel, type_: Type) -> Option<f64> {
    match axis.label_angle {
        Some(angle) => Some(normalize_angle(angle)),
        None if channel == Channel::X && type_.is_discrete() => Some(270.0),
        None => None,
    }
}

/// `defaultLabelBaseline`, for a fixed orient.
fn label_baseline(angle: Option<f64>, orient: &str, channel: Channel) -> Option<&'static str> {
    let angle = angle?;
    match channel {
        Channel::X => {
            if (45.0 < angle && angle < 135.0) || (225.0 < angle && angle < 315.0) {
                Some("middle")
            } else if (angle <= 45.0 || 315.0 <= angle) == (orient == "top") {
                Some("bottom")
            } else {
                Some("top")
            }
        }
        Channel::Y => {
            if angle <= 45.0 || 315.0 <= angle || (135.0..=225.0).contains(&angle) {
                None
            } else if (45.0..=135.0).contains(&angle) == (orient == "left") {
                Some("top")
            } else {
                Some("bottom")
            }
        }
    }
}

/// `defaultLabelAlign`, for a fixed orient.
fn label_align(angle: Option<f64>, orient: &str, channel: Channel) -> Option<&'static str> {
    let angle = angle?;
    let (start, main) = match channel {
        Channel::X => (0.0, "bottom"),
        Channel::Y => (90.0, "left"),
    };
    if (angle + start) % 180.0 == 0.0 {
        return match channel {
            Channel::X => None,
            Channel::Y => Some("center"),
        };
    }
    if (start < angle && angle < 180.0 + start) == (orient == main) {
        Some("left")
    } else {
        Some("right")
    }
}

/// One channel's axis component: Vega axis properties by name.
struct AxisComponent {
    props: Vec<(&'static str, Json)>,
}

impl AxisComponent {
    fn get(&self, name: &str) -> Option<&Json> {
        self.props.iter().find(|(n, _)| *n == name).map(|(_, v)| v)
    }
}

/// `AXIS_PROPERTY_TYPE` for the grid axis: what it keeps of the
/// component.
fn grid_kind(name: &str) -> bool {
    name.starts_with("grid")
        || matches!(
            name,
            "bandPosition"
                | "offset"
                | "tickMinStep"
                | "tickOffset"
                | "tickRound"
                | "tickWidth"
                | "encode"
                | "scale"
                | "tickBand"
                | "tickCount"
                | "tickExtra"
                | "translate"
                | "values"
                | "zindex"
        )
}

/// `parseUnitAxes` and `assembleAxes`.
pub fn parse(unit: &Unit, scales: &[Component], _sizes: &Sizes) -> Result<Vec<vg::Axis>, Error> {
    let mut components: Vec<(Channel, AxisComponent)> = Vec::new();
    for scale in scales {
        let channel = scale.channel;
        let def = unit
            .field_def(channel)
            .expect("a scale channel has a field");
        let axis = match &def.axis {
            Some(None) => continue,
            Some(Some(axis)) => axis.clone(),
            None => Axis::default(),
        };
        let orient = match channel {
            Channel::X => "bottom",
            Channel::Y => "left",
        };
        let angle = label_angle(&axis, channel, def.type_);
        let mut props: Vec<(&'static str, Json)> = Vec::new();
        let mut set = |name: &'static str, value: Option<Json>| {
            if let Some(value) = value {
                props.push((name, value));
            }
        };
        set("scale", Some(json!(scale.name)));
        set("orient", Some(json!(orient)));
        set(
            "grid",
            Some(json!(axis.grid.unwrap_or(!scale.is_discrete()))),
        );
        set(
            "gridScale",
            scales
                .iter()
                .find(|s| s.channel == channel.other())
                .map(|s| json!(s.name)),
        );
        set(
            "labelAlign",
            label_align(angle, orient, channel).map(Json::from),
        );
        set("labelAngle", angle.map(Json::from));
        set(
            "labelBaseline",
            label_baseline(angle, orient, channel).map(Json::from),
        );
        set(
            "labelFlush",
            (channel == Channel::X && matches!(def.type_, Type::Quantitative | Type::Temporal))
                .then(|| json!(true)),
        );
        set(
            "labelOverlap",
            (!def.type_.is_discrete()).then(|| json!(true)),
        );
        set(
            "tickCount",
            (!scale.is_discrete())
                .then(|| json!({"signal": format!("ceil({}/40)", channel.size())})),
        );
        set(
            "title",
            Some(axis.title.clone().unwrap_or_else(|| field::title(def))),
        );
        set(
            "zindex",
            Some(json!(if unit.mark == "rect" && def.type_.is_discrete() {
                1
            } else {
                0
            })),
        );
        components.push((channel, AxisComponent { props }));
    }
    let mut axes = Vec::new();
    for (_, component) in &components {
        if component.get("grid") != Some(&json!(true)) {
            continue;
        }
        let mut axis = json!({});
        for (name, value) in &component.props {
            if grid_kind(name) || *name == "orient" {
                axis[*name] = value.clone();
            }
        }
        axis["domain"] = json!(false);
        axis["labels"] = json!(false);
        axis["aria"] = json!(false);
        axis["maxExtent"] = json!(0);
        axis["minExtent"] = json!(0);
        axis["ticks"] = json!(false);
        axis["zindex"] = component.get("zindex").cloned().unwrap_or(json!(0));
        axes.push(axis);
    }
    for (_, component) in &components {
        let mut axis = json!({});
        axis["grid"] = json!(false);
        for (name, value) in &component.props {
            if name.starts_with("grid") {
                continue;
            }
            if *name == "title" && !crate::compile::truthy(value) {
                continue;
            }
            axis[*name] = value.clone();
        }
        axes.push(axis);
    }
    axes.into_iter()
        .map(|axis| serde_json::from_value(axis).map_err(|e| Error::Spec(e.to_string())))
        .collect()
}
