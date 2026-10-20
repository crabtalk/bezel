//! A `classDef`: the styles it gives a node, and those it gives its text.

use serde::Serialize;

use crate::js;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleClass {
    pub id: String,
    pub styles: Vec<String>,
    pub text_styles: Vec<String>,
}

impl StyleClass {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            styles: Vec::new(),
            text_styles: Vec::new(),
        }
    }

    /// Adds `style`. One that sets a color also styles the text, its first
    /// `fill` as `bgFill`.
    pub fn push(&mut self, style: &str) {
        if style.contains("color") {
            self.text_styles.push(style.replacen("fill", "bgFill", 1));
        }
        self.styles.push(style.to_string());
    }

    /// Adds one `attribute` of a `style` or `classDef` list, without its first
    /// `;`. One that sets a color styles the text with it as `fill`, and its
    /// first `fill` as `bgFill`.
    pub fn push_attribute(&mut self, attribute: &str) {
        let fixed = js::trim(&attribute.replacen(';', "", 1)).to_string();
        if attribute.contains("color") {
            self.text_styles.push(
                fixed
                    .replacen("fill", "bgFill", 1)
                    .replacen("color", "fill", 1),
            );
        }
        self.styles.push(fixed);
    }
}
