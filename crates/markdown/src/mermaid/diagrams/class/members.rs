//! Pele's `diagrams/class/members.ts`.

use serde::Serialize;

use crate::mermaid::{
    diagrams::common::generics::parse_generic_types,
    js,
    util::chars::{code_at, is_line_end, width_at},
};

fn is_visibility(ch: &str) -> bool {
    matches!(ch, "#" | "+" | "~" | "-")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MemberType {
    Method,
    Attribute,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassMember {
    pub id: String,
    pub member_type: MemberType,
    /// `#`, `+`, `~`, `-` or empty.
    pub visibility: String,
    pub classifier: String,
    pub parameters: String,
    pub return_type: String,
}

pub struct DisplayDetails {
    pub display_text: String,
    pub css_style: &'static str,
}

impl ClassMember {
    pub fn new(input: &str, member_type: MemberType) -> Self {
        let mut member = Self {
            id: String::new(),
            member_type,
            visibility: String::new(),
            classifier: String::new(),
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
        let n = input.len();
        let bytes = input.as_bytes();
        let mut start = 0;
        while start < n {
            let mut end = start;
            while end < n && !is_line_end(code_at(input, end)) {
                end += width_at(input, end);
            }
            let mut close = end as isize - 1;
            while close > start as isize && bytes[close as usize] != b')' {
                close -= 1;
            }
            let mut open = close - 1;
            while open > start as isize && bytes[open as usize] != b'(' {
                open -= 1;
            }
            if open > start as isize {
                let (open, close) = (open as usize, close as usize);
                let first = &input[start..start + width_at(input, start)];
                let visible = is_visibility(first) && open > start + 1;
                if visible {
                    self.visibility = first.to_string();
                }
                self.id = input[if visible { start + 1 } else { start }..open].to_string();
                self.parameters = js::trim(&input[open + 1..close]).to_string();
                let mut rest = close + 1;
                let mut classifier = "";
                if rest < n {
                    let next = &input[rest..rest + width_at(input, rest)];
                    if next == "$" || next == "*" || js::trim(next).is_empty() {
                        classifier = js::trim(next);
                        rest += next.len();
                    }
                }
                let mut stop = rest;
                while stop < n && !is_line_end(code_at(input, stop)) {
                    stop += width_at(input, stop);
                }
                self.return_type = js::trim(&input[rest..stop]).to_string();
                if classifier.is_empty()
                    && (self.return_type.ends_with('$') || self.return_type.ends_with('*'))
                {
                    let cut = self.return_type.len() - 1;
                    classifier = if self.return_type.ends_with('$') {
                        "$"
                    } else {
                        "*"
                    };
                    self.return_type.truncate(cut);
                }
                self.classifier = classifier.to_string();
                return true;
            }
            start = end + width_at(input, end);
        }
        self.member_type = MemberType::Attribute;
        false
    }

    fn parse_attribute(&mut self, input: &str) {
        let first = input.get(..1).unwrap_or("");
        if is_visibility(first) {
            self.visibility = first.to_string();
        }
        if input.ends_with('$') || input.ends_with('*') {
            self.classifier = input[input.len() - 1..].to_string();
        }
        let from = if self.visibility.is_empty() { 0 } else { 1 };
        let to = if self.classifier.is_empty() {
            input.len()
        } else {
            input.len() - 1
        };
        self.id = input[from.min(to)..from.max(to)].to_string();
    }

    pub fn display_details(&self) -> DisplayDetails {
        let mut display_text = self.visibility.clone() + &parse_generic_types(&self.id);
        if self.member_type == MemberType::Method {
            display_text += &format!("({})", parse_generic_types(js::trim(&self.parameters)));
            if !self.return_type.is_empty() {
                display_text += " : ";
                display_text += &parse_generic_types(&self.return_type);
            }
        }
        DisplayDetails {
            display_text: js::trim(&display_text).to_string(),
            css_style: self.parse_classifier(),
        }
    }

    pub fn parse_classifier(&self) -> &'static str {
        match self.classifier.as_str() {
            "*" => "font-style:italic;",
            "$" => "text-decoration:underline;",
            _ => "",
        }
    }
}
