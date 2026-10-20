//! Pele's `diagrams/class/members.ts`.

use serde::{Serialize, Serializer};
use std::fmt::Write as _;

use crate::{
    diagrams::common::generics::parse_generic_types,
    js,
    util::chars::{is_line_end, is_space},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MemberType {
    Method,
    Attribute,
}

/// The marker before a member: `+`, `-`, `#` or `~`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Private,
    Protected,
    Package,
}

impl Visibility {
    fn parse(c: char) -> Option<Self> {
        match c {
            '+' => Some(Self::Public),
            '-' => Some(Self::Private),
            '#' => Some(Self::Protected),
            '~' => Some(Self::Package),
            _ => None,
        }
    }

    pub fn marker(self) -> &'static str {
        match self {
            Self::Public => "+",
            Self::Private => "-",
            Self::Protected => "#",
            Self::Package => "~",
        }
    }
}

/// The marker after a member: `$` or `*`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Classifier {
    Static,
    Abstract,
}

impl Classifier {
    fn parse(c: char) -> Option<Self> {
        match c {
            '$' => Some(Self::Static),
            '*' => Some(Self::Abstract),
            _ => None,
        }
    }

    pub fn marker(self) -> &'static str {
        match self {
            Self::Static => "$",
            Self::Abstract => "*",
        }
    }

    pub fn css_style(self) -> &'static str {
        match self {
            Self::Static => "text-decoration:underline;",
            Self::Abstract => "font-style:italic;",
        }
    }
}

/// A marker as its character, and no marker as the empty string.
fn marker<S: Serializer>(marker: Option<&'static str>, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(marker.unwrap_or(""))
}

fn visibility<S: Serializer>(value: &Option<Visibility>, s: S) -> Result<S::Ok, S::Error> {
    marker(value.map(Visibility::marker), s)
}

fn classifier<S: Serializer>(value: &Option<Classifier>, s: S) -> Result<S::Ok, S::Error> {
    marker(value.map(Classifier::marker), s)
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassMember {
    pub id: String,
    pub member_type: MemberType,
    #[serde(serialize_with = "visibility")]
    pub visibility: Option<Visibility>,
    #[serde(serialize_with = "classifier")]
    pub classifier: Option<Classifier>,
    pub parameters: String,
    pub return_type: String,
}

pub struct DisplayDetails {
    pub display_text: String,
    pub css_style: &'static str,
}

/// The end of the line at `from` of `input`.
fn line_end(input: &str, from: usize) -> usize {
    input[from..]
        .find(|c: char| is_line_end(c))
        .map_or(input.len(), |at| from + at)
}

impl ClassMember {
    pub fn new(input: &str, member_type: MemberType) -> Self {
        let mut member = Self {
            id: String::new(),
            member_type,
            visibility: None,
            classifier: None,
            parameters: String::new(),
            return_type: String::new(),
        };
        // Mermaid throws a TypeError for a method with no opening
        // parenthesis, such as `run)`. It is kept here as an attribute, so it
        // is shown as written.
        if member_type != MemberType::Method || !member.parse_method(input) {
            member.parse_attribute(input);
        }
        member.id = if member.id.starts_with(' ') {
            format!(" {}", js::trim(&member.id))
        } else {
            js::trim(&member.id).to_string()
        };
        member
    }

    /// The same split as Mermaid's
    /// `/([#+~-])?(.+)\((.*)\)([\s$*])?(.*)([$*])?/`, found in one pass: the
    /// first line with an opening parenthesis after its first character and a
    /// closing one later.
    fn parse_method(&mut self, input: &str) -> bool {
        let mut start = 0;
        while start < input.len() {
            let end = line_end(input, start);
            let line = &input[start..end];
            let parens = line
                .rfind(')')
                .filter(|&close| close > 0)
                .and_then(|close| {
                    line[..close]
                        .rfind('(')
                        .filter(|&open| open > 0)
                        .map(|open| (start + open, start + close))
                });
            let Some((open, close)) = parens else {
                start = end + input[end..].chars().next().map_or(1, char::len_utf8);
                continue;
            };
            let visibility = line
                .chars()
                .next()
                .and_then(Visibility::parse)
                .filter(|_| open > start + 1);
            self.visibility = visibility;
            let id_start = if visibility.is_some() {
                start + 1
            } else {
                start
            };
            self.id = input[id_start..open].to_string();
            self.parameters = js::trim(&input[open + 1..close]).to_string();
            let mut rest = close + 1;
            if let Some(c) = input[rest..].chars().next()
                && (Classifier::parse(c).is_some() || is_space(c))
            {
                self.classifier = Classifier::parse(c);
                rest += c.len_utf8();
            }
            self.return_type = js::trim(&input[rest..line_end(input, rest)]).to_string();
            if self.classifier.is_none()
                && let Some(c) = self.return_type.chars().next_back()
                && let Some(classifier) = Classifier::parse(c)
            {
                self.classifier = Some(classifier);
                self.return_type.pop();
            }
            return true;
        }
        self.member_type = MemberType::Attribute;
        false
    }

    fn parse_attribute(&mut self, input: &str) {
        self.visibility = input.chars().next().and_then(Visibility::parse);
        self.classifier = input.chars().next_back().and_then(Classifier::parse);
        let from = usize::from(self.visibility.is_some());
        let to = input.len() - usize::from(self.classifier.is_some());
        self.id = input[from.min(to)..from.max(to)].to_string();
    }

    pub fn display_details(&self) -> DisplayDetails {
        let mut display_text = self.visibility.map_or("", Visibility::marker).to_string()
            + &parse_generic_types(&self.id);
        if self.member_type == MemberType::Method {
            let _ = write!(
                display_text,
                "({})",
                parse_generic_types(js::trim(&self.parameters))
            );
            if !self.return_type.is_empty() {
                display_text += " : ";
                let _ = write!(display_text, "{}", parse_generic_types(&self.return_type));
            }
        }
        DisplayDetails {
            display_text: js::trim(&display_text).to_string(),
            css_style: self.classifier.map_or("", Classifier::css_style),
        }
    }
}
