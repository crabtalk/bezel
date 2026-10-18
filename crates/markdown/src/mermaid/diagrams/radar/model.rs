//! Pele's `diagrams/radar/model.ts`.

use std::collections::HashMap;

use serde::Serialize;

use super::parser::{OptionValue, RadarAst};
use crate::mermaid::{
    detect::Kind,
    error::Error,
    js::{self, Lazy},
};

const MAX_TICKS: f64 = 32.0;

static RE_LEAD: Lazy = Lazy::new(r"^\s+", "g");
static RE_INDENT: Lazy = Lazy::new(r"\n\s+", "g");

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RadarAxis {
    pub name: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RadarCurve {
    pub name: String,
    pub label: String,
    #[serde(serialize_with = "js::json_numbers")]
    pub entries: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RadarOptions {
    pub show_legend: bool,
    #[serde(serialize_with = "js::json_number")]
    pub ticks: f64,
    #[serde(serialize_with = "js::json_number_opt")]
    pub max: Option<f64>,
    #[serde(serialize_with = "js::json_number")]
    pub min: f64,
    pub graticule: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RadarModel {
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acc_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acc_descr: Option<String>,
    pub axes: Vec<RadarAxis>,
    pub curves: Vec<RadarCurve>,
    pub options: RadarOptions,
}

fn invalid(message: impl Into<String>) -> Error {
    Error::semantic(message).of(Kind::Radar)
}

pub fn build_radar(ast: RadarAst, title: Option<&str>) -> Result<RadarModel, Error> {
    let axes: Vec<RadarAxis> = ast
        .axes
        .into_iter()
        .map(|axis| RadarAxis {
            label: axis.label.unwrap_or_else(|| axis.name.clone()),
            name: axis.name,
        })
        .collect();

    let mut curves = Vec::with_capacity(ast.curves.len());
    for curve in ast.curves {
        let entries = if curve.entries[0].axis.is_none() {
            curve.entries.iter().map(|entry| entry.value).collect()
        } else {
            if axes.is_empty() {
                return Err(invalid(
                    "Axes must be populated before curves for reference entries",
                ));
            }
            let mut by_axis: HashMap<&str, f64> = HashMap::new();
            for entry in &curve.entries {
                if let Some(axis) = &entry.axis {
                    by_axis.entry(axis).or_insert(entry.value);
                }
            }
            let mut entries = Vec::with_capacity(axes.len());
            for axis in &axes {
                match by_axis.get(axis.name.as_str()) {
                    Some(&value) => entries.push(value),
                    None => return Err(invalid(format!("Missing entry for axis {}", axis.label))),
                }
            }
            entries
        };
        curves.push(RadarCurve {
            label: curve.label.unwrap_or_else(|| curve.name.clone()),
            name: curve.name,
            entries,
        });
    }

    let mut options = RadarOptions {
        show_legend: true,
        ticks: 5.0,
        max: None,
        min: 0.0,
        graticule: "circle".into(),
    };
    for option in ast.options {
        match (option.name.as_str(), option.value) {
            ("showLegend", OptionValue::Bool(value)) => options.show_legend = value,
            ("graticule", OptionValue::Text(value)) => options.graticule = value,
            ("ticks", OptionValue::Number(value)) => options.ticks = value,
            ("max", OptionValue::Number(value)) => options.max = Some(value),
            ("min", OptionValue::Number(value)) => options.min = value,
            _ => {}
        }
    }
    if options.ticks > MAX_TICKS {
        options.ticks = MAX_TICKS;
    }

    Ok(RadarModel {
        kind: "radar",
        title: ast
            .title
            .filter(|title| !title.is_empty())
            .or_else(|| title.map(str::to_string)),
        acc_title: ast
            .acc_title
            .filter(|text| !text.is_empty())
            .map(|text| RE_LEAD.replace(&text, |_| String::new())),
        acc_descr: ast
            .acc_descr
            .filter(|text| !text.is_empty())
            .map(|text| RE_INDENT.replace(&text, |_| "\n".into())),
        axes,
        curves,
        options,
    })
}
