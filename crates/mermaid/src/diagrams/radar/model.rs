//! Pele's `diagrams/radar/model.ts`.

use crate::diagrams::common::db::Titles;
use std::collections::HashMap;

use serde::Serialize;

use super::parser::{OptionAst, RadarAst};
use crate::{
    detect::Kind,
    error::Error,
    js::{self},
};

const MAX_TICKS: f64 = 32.0;

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

/// The rings marking the ticks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Graticule {
    Circle,
    Polygon,
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
    pub graticule: Graticule,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RadarModel {
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(flatten)]
    pub titles: Titles,
    pub axes: Vec<RadarAxis>,
    pub curves: Vec<RadarCurve>,
    pub options: RadarOptions,
}

impl RadarModel {
    pub fn new(ast: RadarAst, title: Option<&str>) -> Result<Self, Error> {
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
                    return Err(Error::semantic(
                        "Axes must be populated before curves for reference entries",
                    )
                    .of(Kind::Radar));
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
                        None => {
                            return Err(Error::semantic(format!(
                                "Missing entry for axis {}",
                                axis.label
                            ))
                            .of(Kind::Radar));
                        }
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
            graticule: Graticule::Circle,
        };
        for option in ast.options {
            match option {
                OptionAst::ShowLegend(value) => options.show_legend = value,
                OptionAst::Graticule(value) => options.graticule = value,
                OptionAst::Ticks(value) => options.ticks = value,
                OptionAst::Max(value) => options.max = Some(value),
                OptionAst::Min(value) => options.min = value,
            }
        }
        if options.ticks > MAX_TICKS {
            options.ticks = MAX_TICKS;
        }

        Ok(Self {
            kind: "radar",
            titles: ast.header.titles(title),
            axes,
            curves,
            options,
        })
    }
}
