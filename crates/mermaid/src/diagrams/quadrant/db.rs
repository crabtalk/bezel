//! Pele's `diagrams/quadrant/db.ts`.

use indexmap::IndexMap;
use serde::Serialize;

use crate::{detect::Kind, diagrams::common::db::Titles, error::Error, js};

fn is_number(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit())
}

fn is_hex(value: &str) -> bool {
    let hex = value.strip_prefix('#').unwrap_or(value);
    matches!(hex.len(), 3 | 6) && hex.bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_pixels(value: &str) -> bool {
    value.strip_suffix("px").is_some_and(is_number)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TextKind {
    Text,
    Markdown,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct QuadrantText {
    pub text: String,
    #[serde(rename = "type")]
    pub kind: TextKind,
}

impl QuadrantText {
    fn trimmed(self) -> Self {
        Self {
            text: js::trim(&self.text).to_string(),
            kind: self.kind,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PointStyle {
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "js::json_number_opt"
    )]
    pub radius: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke_width: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuadrantPoint {
    #[serde(serialize_with = "js::json_number")]
    pub x: f64,
    #[serde(serialize_with = "js::json_number")]
    pub y: f64,
    pub text: QuadrantText,
    pub class_name: String,
    #[serde(flatten)]
    pub style: PointStyle,
}

/// Pele's `QuadrantDb`, which is its model.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(tag = "type", rename = "quadrantChart", rename_all = "camelCase")]
pub struct QuadrantModel {
    #[serde(flatten)]
    pub titles: Titles,
    /// Quadrants 1 to 4: top right, top left, bottom left, bottom right.
    pub quadrants: [Option<QuadrantText>; 4],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x_axis_left: Option<QuadrantText>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x_axis_right: Option<QuadrantText>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y_axis_bottom: Option<QuadrantText>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y_axis_top: Option<QuadrantText>,
    pub points: Vec<QuadrantPoint>,
    pub classes: IndexMap<String, PointStyle>,
}

fn invalid(style: &str, value: &str, kind: &str) -> Error {
    Error::semantic(format!(
        "value for {style} {value} is invalid, please use a valid {kind}"
    ))
    .of(Kind::QuadrantChart)
}

/// The first two parts of Mermaid's `style.trim().split(/\s*:\s*/)`, found in
/// one pass.
fn key_value(style: &str) -> (&str, &str) {
    let s = js::trim(style);
    let Some(colon) = s.find(':') else {
        return (s, "undefined");
    };
    let rest = js::trim_start(&s[colon + 1..]);
    let value = match rest.find(':') {
        None => rest,
        Some(next) => js::trim_end(&rest[..next]),
    };
    (js::trim_end(&s[..colon]), value)
}

impl PointStyle {
    pub fn parse(styles: &[String]) -> Result<Self, Error> {
        let mut out = Self::default();
        for style in styles {
            let (key, value) = key_value(style);
            match key {
                "radius" => {
                    if !is_number(value) {
                        return Err(invalid(key, value, "number"));
                    }
                    out.radius = Some(js::parse_int(value, 10));
                }
                "color" => {
                    if !is_hex(value) {
                        return Err(invalid(key, value, "hex code"));
                    }
                    out.color = Some(value.to_string());
                }
                "stroke-color" => {
                    if !is_hex(value) {
                        return Err(invalid(key, value, "hex code"));
                    }
                    out.stroke_color = Some(value.to_string());
                }
                "stroke-width" => {
                    if !is_pixels(value) {
                        return Err(invalid(key, value, "number of pixels (eg. 10px)"));
                    }
                    out.stroke_width = Some(value.to_string());
                }
                _ => {
                    return Err(
                        Error::semantic(format!("style named {key} is not supported."))
                            .of(Kind::QuadrantChart),
                    );
                }
            }
        }
        Ok(out)
    }
}

impl QuadrantModel {
    /// Quadrant `k`, counting from 0.
    pub fn set_quadrant_text(&mut self, k: usize, label: QuadrantText) {
        self.quadrants[k] = Some(label.trimmed());
    }

    pub fn set_x_axis_left_text(&mut self, label: QuadrantText) {
        self.x_axis_left = Some(label.trimmed());
    }

    pub fn set_x_axis_right_text(&mut self, label: QuadrantText) {
        self.x_axis_right = Some(label.trimmed());
    }

    pub fn set_y_axis_bottom_text(&mut self, label: QuadrantText) {
        self.y_axis_bottom = Some(label.trimmed());
    }

    pub fn set_y_axis_top_text(&mut self, label: QuadrantText) {
        self.y_axis_top = Some(label.trimmed());
    }

    pub fn add_point(
        &mut self,
        label: QuadrantText,
        class_name: String,
        x: &str,
        y: &str,
        styles: &[String],
    ) -> Result<(), Error> {
        let style = PointStyle::parse(styles)?;
        self.points.push(QuadrantPoint {
            x: js::number(x),
            y: js::number(y),
            text: label.trimmed(),
            class_name,
            style,
        });
        Ok(())
    }

    pub fn add_class(&mut self, class_name: String, styles: &[String]) -> Result<(), Error> {
        let style = PointStyle::parse(styles)?;
        self.classes.insert(class_name, style);
        Ok(())
    }
}
