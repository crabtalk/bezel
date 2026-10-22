//! The title and accessibility text every diagram's database keeps. Pele's
//! `diagrams/common/commonDb.ts`.

use crate::{string, util::chars};
use regex::Regex;
use serde::Serialize;
use std::sync::LazyLock;

use crate::svg::root::Accessible;

static RE_INDENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"\n[{}]+", chars::SPACE)).expect("pattern"));

/// `accTitle` text without its leading whitespace.
pub fn acc_title(text: &str) -> String {
    string::trim_start(text).to_string()
}

/// `accDescr` text without the indentation of its lines after the first.
pub fn acc_descr(text: &str) -> String {
    RE_INDENT.replace_all(text, "\n").into_owned()
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
