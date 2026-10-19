//! Pele's `diagrams/quadrant/db.ts`.

use indexmap::IndexMap;
use serde::Serialize;

use crate::{
    detect::Kind,
    error::Error,
    js::{self, Lazy},
};

static RE_NUMBER: Lazy = Lazy::new(r"^\d+$", "");
static RE_HEX: Lazy = Lazy::new(r"^#?([\dA-Fa-f]{6}|[\dA-Fa-f]{3})$", "");
static RE_PIXELS: Lazy = Lazy::new(r"^\d+px$", "");
static RE_LEAD: Lazy = Lazy::new(r"^\s+", "g");
static RE_INDENT: Lazy = Lazy::new(r"\n\s+", "g");

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct QuadrantText {
    pub text: String,
    #[serde(rename = "type")]
    pub kind: &'static str,
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
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuadrantModel {
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acc_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acc_descr: Option<String>,
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

pub fn parse_styles(styles: &[String]) -> Result<PointStyle, Error> {
    let mut out = PointStyle::default();
    for style in styles {
        let (key, value) = key_value(style);
        match key {
            "radius" => {
                if !RE_NUMBER.test(value) {
                    return Err(invalid(key, value, "number"));
                }
                out.radius = Some(js::parse_int(value, 10));
            }
            "color" => {
                if !RE_HEX.test(value) {
                    return Err(invalid(key, value, "hex code"));
                }
                out.color = Some(value.to_string());
            }
            "stroke-color" => {
                if !RE_HEX.test(value) {
                    return Err(invalid(key, value, "hex code"));
                }
                out.stroke_color = Some(value.to_string());
            }
            "stroke-width" => {
                if !RE_PIXELS.test(value) {
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

fn trimmed(label: QuadrantText) -> QuadrantText {
    QuadrantText {
        text: js::trim(&label.text).to_string(),
        kind: label.kind,
    }
}

impl Default for QuadrantModel {
    fn default() -> Self {
        Self {
            kind: "quadrantChart",
            title: None,
            acc_title: None,
            acc_descr: None,
            quadrants: [None, None, None, None],
            x_axis_left: None,
            x_axis_right: None,
            y_axis_bottom: None,
            y_axis_top: None,
            points: Vec::new(),
            classes: IndexMap::new(),
        }
    }
}

impl QuadrantModel {
    /// Quadrant `k`, counting from 0.
    pub fn set_quadrant_text(&mut self, k: usize, label: QuadrantText) {
        self.quadrants[k] = Some(trimmed(label));
    }

    pub fn set_x_axis_left_text(&mut self, label: QuadrantText) {
        self.x_axis_left = Some(trimmed(label));
    }

    pub fn set_x_axis_right_text(&mut self, label: QuadrantText) {
        self.x_axis_right = Some(trimmed(label));
    }

    pub fn set_y_axis_bottom_text(&mut self, label: QuadrantText) {
        self.y_axis_bottom = Some(trimmed(label));
    }

    pub fn set_y_axis_top_text(&mut self, label: QuadrantText) {
        self.y_axis_top = Some(trimmed(label));
    }

    pub fn add_point(
        &mut self,
        label: QuadrantText,
        class_name: String,
        x: &str,
        y: &str,
        styles: &[String],
    ) -> Result<(), Error> {
        let style = parse_styles(styles)?;
        self.points.push(QuadrantPoint {
            x: js::number(x),
            y: js::number(y),
            text: trimmed(label),
            class_name,
            style,
        });
        Ok(())
    }

    pub fn add_class(&mut self, class_name: String, styles: &[String]) -> Result<(), Error> {
        let style = parse_styles(styles)?;
        self.classes.insert(class_name, style);
        Ok(())
    }

    pub fn set_diagram_title(&mut self, text: &str) {
        self.title = Some(text.to_string());
    }

    pub fn set_acc_title(&mut self, text: &str) {
        self.acc_title = Some(RE_LEAD.replace(text, |_| String::new()));
    }

    pub fn set_acc_description(&mut self, text: &str) {
        self.acc_descr = Some(RE_INDENT.replace(text, |_| "\n".into()));
    }
}
