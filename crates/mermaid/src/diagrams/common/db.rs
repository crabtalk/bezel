//! The title and accessibility text every diagram's database keeps. Pele's
//! `diagrams/common/commonDb.ts`.

use serde::Serialize;

use crate::{
    js::{self, Lazy},
    svg::root::Accessible,
};

static RE_INDENT: Lazy = Lazy::new(r"\n\s+", "g");

/// `accTitle` text without its leading whitespace.
pub fn acc_title(text: &str) -> String {
    js::trim_start(text).to_string()
}

/// `accDescr` text without the indentation of its lines after the first.
pub fn acc_descr(text: &str) -> String {
    RE_INDENT.replace(text, |_| "\n".into())
}

/// A diagram's title and accessibility text, each absent from a model's JSON
/// until set.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Titles {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acc_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acc_descr: Option<String>,
}

impl Titles {
    pub fn set_title(&mut self, text: &str) {
        self.title = Some(text.to_string());
    }

    pub fn set_acc_title(&mut self, text: &str) {
        self.acc_title = Some(acc_title(text));
    }

    pub fn set_acc_descr(&mut self, text: &str) {
        self.acc_descr = Some(acc_descr(text));
    }

    /// What a screen reader announces of the diagram.
    pub fn accessible(&self) -> Accessible<'_> {
        Accessible {
            acc_title: self.acc_title.as_deref(),
            acc_descr: self.acc_descr.as_deref(),
        }
    }
}
