//! Pele's `diagrams/xychart/db.ts`.

use serde::{Deserialize, Serialize, Serializer};

use crate::{
    diagrams::common::db::Titles,
    js::{self, JsonNumber},
};

#[derive(Clone, Debug, PartialEq)]
pub struct DataPoint {
    pub value: f64,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum XAxis {
    Band {
        title: String,
        categories: Vec<String>,
    },
    Linear {
        title: String,
        #[serde(serialize_with = "js::json_number")]
        min: f64,
        #[serde(serialize_with = "js::json_number")]
        max: f64,
    },
}

impl XAxis {
    pub fn title(&self) -> &str {
        match self {
            Self::Band { title, .. } | Self::Linear { title, .. } => title,
        }
    }

    fn title_mut(&mut self) -> &mut String {
        match self {
            Self::Band { title, .. } | Self::Linear { title, .. } => title,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename = "linear")]
pub struct LinearAxis {
    pub title: String,
    #[serde(serialize_with = "js::json_number")]
    pub min: f64,
    #[serde(serialize_with = "js::json_number")]
    pub max: f64,
}

/// Each value with its place on the x axis: a category, or a number written
/// as text. A category with no value has `None`.
pub type PlotData = Vec<(String, Option<f64>)>;

fn plot_data_json<S: Serializer>(data: &PlotData, s: S) -> Result<S::Ok, S::Error> {
    s.collect_seq(
        data.iter()
            .map(|(place, value)| (place, value.map(JsonNumber))),
    )
}

/// Which way the bars run: up from the category axis, or across from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Orientation {
    Vertical,
    Horizontal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PlotKind {
    Line,
    Bar,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plot {
    #[serde(rename = "type")]
    pub kind: PlotKind,
    pub title: String,
    #[serde(serialize_with = "plot_data_json")]
    pub data: PlotData,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub point_labels: Option<Vec<String>>,
}

/// Pele's `XyChartDb`, which is its model.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename = "xychart", rename_all = "camelCase")]
pub struct XyChartModel {
    #[serde(flatten)]
    pub titles: Titles,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orientation: Option<Orientation>,
    pub x_axis: XAxis,
    pub y_axis: LinearAxis,
    pub has_set_x_axis: bool,
    /// False while the y range comes from the data and not from a `y-axis`
    /// statement.
    pub has_set_y_axis: bool,
    pub plots: Vec<Plot>,
}

impl Default for XyChartModel {
    fn default() -> Self {
        Self {
            titles: Titles::default(),
            orientation: None,
            x_axis: XAxis::Band {
                title: String::new(),
                categories: Vec::new(),
            },
            y_axis: LinearAxis {
                title: String::new(),
                min: f64::INFINITY,
                max: f64::NEG_INFINITY,
            },
            has_set_x_axis: false,
            has_set_y_axis: false,
            plots: Vec::new(),
        }
    }
}

impl XyChartModel {
    pub fn set_orientation(&mut self, orientation: &str) {
        self.orientation = Some(if orientation == "horizontal" {
            Orientation::Horizontal
        } else {
            Orientation::Vertical
        });
    }

    pub fn set_x_axis_title(&mut self, title: &str) {
        *self.x_axis.title_mut() = js::trim(title).to_string();
    }

    pub fn set_x_axis_range_data(&mut self, min: f64, max: f64) {
        self.x_axis = XAxis::Linear {
            title: self.x_axis.title().to_string(),
            min,
            max,
        };
        self.has_set_x_axis = true;
    }

    pub fn set_x_axis_band(&mut self, categories: &[String]) {
        self.x_axis = XAxis::Band {
            title: self.x_axis.title().to_string(),
            categories: categories.iter().map(|c| js::trim(c).to_string()).collect(),
        };
        self.has_set_x_axis = true;
    }

    pub fn set_y_axis_title(&mut self, title: &str) {
        self.y_axis.title = js::trim(title).to_string();
    }

    pub fn set_y_axis_range_data(&mut self, min: f64, max: f64) {
        self.y_axis = LinearAxis {
            title: self.y_axis.title.clone(),
            min,
            max,
        };
        self.has_set_y_axis = true;
    }

    fn plot_data(&mut self, mut values: Vec<f64>) -> PlotData {
        if values.is_empty() {
            return Vec::new();
        }
        // The first series fixes the x range when no x-axis statement did.
        if !self.has_set_x_axis {
            self.set_x_axis_range_data(1.0, values.len() as f64);
        }
        if let XAxis::Band { categories, .. } = &self.x_axis
            && values.len() > categories.len()
        {
            values.truncate(categories.len());
        }
        if !self.has_set_y_axis {
            let (mut min, mut max) = (self.y_axis.min, self.y_axis.max);
            for &value in &values {
                if value < min {
                    min = value;
                }
                if value > max {
                    max = value;
                }
            }
            self.y_axis = LinearAxis {
                title: self.y_axis.title.clone(),
                min,
                max,
            };
        }
        match &self.x_axis {
            XAxis::Band { categories, .. } => categories
                .iter()
                .enumerate()
                .map(|(i, category)| (category.clone(), values.get(i).copied()))
                .collect(),
            XAxis::Linear { min, max, .. } => {
                if values.len() == 1 {
                    return vec![(js::num_str(*min), Some(values[0]))];
                }
                let step = (max - min) / (values.len() - 1) as f64;
                values
                    .iter()
                    .enumerate()
                    .map(|(i, &value)| (js::num_str(min + i as f64 * step), Some(value)))
                    .collect()
            }
        }
    }

    pub fn add_plot(&mut self, kind: PlotKind, title: &str, data: &[DataPoint]) {
        let values = data.iter().map(|d| d.value).collect();
        let mut plot = Plot {
            kind,
            title: js::trim(title).to_string(),
            data: self.plot_data(values),
            point_labels: None,
        };
        // Mermaid accepts labels on the points of a bar series and ignores
        // them.
        if kind == PlotKind::Line && data.iter().any(|d| !js::trim(&d.label).is_empty()) {
            plot.point_labels = Some(
                data.iter()
                    .map(|d| js::trim(&d.label).to_string())
                    .collect(),
            );
        }
        self.plots.push(plot);
    }
}
