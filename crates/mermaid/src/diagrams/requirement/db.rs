//! Pele's `diagrams/requirement/db.ts`.

use indexmap::IndexMap;
use serde::Serialize;

use super::types::{Relation, Requirement, RequirementClass, RequirementElement};
use crate::{
    detect::Kind,
    error::Error,
    js::{self, Lazy},
};

pub mod requirement_type {
    pub const REQUIREMENT: &str = "Requirement";
    pub const FUNCTIONAL_REQUIREMENT: &str = "Functional Requirement";
    pub const INTERFACE_REQUIREMENT: &str = "Interface Requirement";
    pub const PERFORMANCE_REQUIREMENT: &str = "Performance Requirement";
    pub const PHYSICAL_REQUIREMENT: &str = "Physical Requirement";
    pub const DESIGN_CONSTRAINT: &str = "Design Constraint";
}

pub mod risk_level {
    pub const LOW_RISK: &str = "Low";
    pub const MED_RISK: &str = "Medium";
    pub const HIGH_RISK: &str = "High";
}

pub mod verify_type {
    pub const VERIFY_ANALYSIS: &str = "Analysis";
    pub const VERIFY_DEMONSTRATION: &str = "Demonstration";
    pub const VERIFY_INSPECTION: &str = "Inspection";
    pub const VERIFY_TEST: &str = "Test";
}

pub mod relationships {
    pub const CONTAINS: &str = "contains";
    pub const COPIES: &str = "copies";
    pub const DERIVES: &str = "derives";
    pub const SATISFIES: &str = "satisfies";
    pub const VERIFIES: &str = "verifies";
    pub const REFINES: &str = "refines";
    pub const TRACES: &str = "traces";
}

/// Applying a class copies its declarations onto each node, so a short text
/// can ask for hundreds of millions of copies. Past this many the diagram is
/// refused.
const MAX_STYLE_COPIES: usize = 1_000_000;

static RE_INDENT: Lazy = Lazy::new(r"\n\s+", "g");

/// Pele's `RequirementDb`, serialized with the fields it keeps between
/// statements as Pele's is.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequirementModel {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub direction: String,
    pub requirements: IndexMap<String, Requirement>,
    pub elements: IndexMap<String, RequirementElement>,
    pub relations: Vec<Relation>,
    pub classes: IndexMap<String, RequirementClass>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acc_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acc_descr: Option<String>,
    // The grammar reports a body's fields before the requirement or element
    // they belong to.
    new_id: String,
    new_text: String,
    new_risk: String,
    new_verify_method: String,
    new_element_type: String,
    new_doc_ref: String,
    style_copies: usize,
}

impl Default for RequirementModel {
    fn default() -> Self {
        Self {
            kind: "requirement",
            direction: "TB".into(),
            requirements: IndexMap::new(),
            elements: IndexMap::new(),
            relations: Vec::new(),
            classes: IndexMap::new(),
            title: None,
            acc_title: None,
            acc_descr: None,
            new_id: String::new(),
            new_text: String::new(),
            new_risk: String::new(),
            new_verify_method: String::new(),
            new_element_type: String::new(),
            new_doc_ref: String::new(),
            style_copies: 0,
        }
    }
}

/// The styles and classes of a requirement or an element.
struct NodeStyles<'a> {
    css_styles: &'a mut Vec<String>,
    classes: &'a mut Vec<String>,
}

fn copy(count: &mut usize, target: &mut Vec<String>, styles: &[String]) -> Result<(), Error> {
    *count += styles.len();
    if *count > MAX_STYLE_COPIES {
        return Err(
            Error::limit("Requirement diagram applies too many style declarations.")
                .of(Kind::Requirement),
        );
    }
    target.extend(styles.iter().cloned());
    Ok(())
}

impl RequirementModel {
    pub fn set_direction(&mut self, dir: &str) {
        self.direction = dir.to_string();
    }

    pub fn add_requirement(&mut self, name: &str, kind: &str) {
        if !self.requirements.contains_key(name) {
            let requirement = Requirement {
                name: name.to_string(),
                kind: kind.to_string(),
                requirement_id: self.new_id.clone(),
                text: self.new_text.clone(),
                risk: self.new_risk.clone(),
                verify_method: self.new_verify_method.clone(),
                css_styles: Vec::new(),
                classes: vec!["default".into()],
            };
            self.requirements.insert(name.to_string(), requirement);
        }
        self.new_id.clear();
        self.new_text.clear();
        self.new_risk.clear();
        self.new_verify_method.clear();
    }

    pub fn set_new_req_id(&mut self, id: &str) {
        self.new_id = id.to_string();
    }

    pub fn set_new_req_text(&mut self, text: &str) {
        self.new_text = text.to_string();
    }

    pub fn set_new_req_risk(&mut self, risk: &str) {
        self.new_risk = risk.to_string();
    }

    pub fn set_new_req_verify_method(&mut self, verify_method: &str) {
        self.new_verify_method = verify_method.to_string();
    }

    pub fn add_element(&mut self, name: &str) {
        if !self.elements.contains_key(name) {
            let element = RequirementElement {
                name: name.to_string(),
                kind: self.new_element_type.clone(),
                doc_ref: self.new_doc_ref.clone(),
                css_styles: Vec::new(),
                classes: vec!["default".into()],
            };
            self.elements.insert(name.to_string(), element);
        }
        self.new_element_type.clear();
        self.new_doc_ref.clear();
    }

    pub fn set_new_element_type(&mut self, kind: &str) {
        self.new_element_type = kind.to_string();
    }

    pub fn set_new_element_doc_ref(&mut self, doc_ref: &str) {
        self.new_doc_ref = doc_ref.to_string();
    }

    pub fn add_relationship(&mut self, kind: &str, src: &str, dst: &str) {
        self.relations.push(Relation {
            kind: kind.to_string(),
            src: src.to_string(),
            dst: dst.to_string(),
        });
    }

    fn node<'a>(
        requirements: &'a mut IndexMap<String, Requirement>,
        elements: &'a mut IndexMap<String, RequirementElement>,
        id: &str,
    ) -> Option<NodeStyles<'a>> {
        if let Some(requirement) = requirements.get_mut(id) {
            return Some(NodeStyles {
                css_styles: &mut requirement.css_styles,
                classes: &mut requirement.classes,
            });
        }
        elements.get_mut(id).map(|element| NodeStyles {
            css_styles: &mut element.css_styles,
            classes: &mut element.classes,
        })
    }

    pub fn set_css_style(&mut self, ids: &[&str], styles: &[String]) -> Result<(), Error> {
        for id in ids {
            // Mermaid stops at the first unknown id, leaving the rest of the
            // list unstyled.
            let Some(node) = Self::node(&mut self.requirements, &mut self.elements, id) else {
                return Ok(());
            };
            for s in styles {
                let parts: Vec<String> = if s.contains(',') {
                    s.split(',').map(str::to_string).collect()
                } else {
                    vec![s.clone()]
                };
                copy(&mut self.style_copies, node.css_styles, &parts)?;
            }
        }
        Ok(())
    }

    pub fn set_class(&mut self, ids: &[&str], class_names: &[&str]) -> Result<(), Error> {
        for id in ids {
            let Some(node) = Self::node(&mut self.requirements, &mut self.elements, id) else {
                continue;
            };
            for name in class_names {
                node.classes.push(name.to_string());
                if let Some(class) = self.classes.get(*name) {
                    copy(&mut self.style_copies, node.css_styles, &class.styles)?;
                }
            }
        }
        Ok(())
    }

    pub fn define_class(&mut self, ids: &[&str], style: &[String]) -> Result<(), Error> {
        for id in ids {
            let class = self
                .classes
                .entry(id.to_string())
                .or_insert_with(|| RequirementClass {
                    id: id.to_string(),
                    styles: Vec::new(),
                    text_styles: Vec::new(),
                });
            for s in style {
                if s.contains("color") {
                    class.text_styles.push(s.replacen("fill", "bgFill", 1));
                }
                class.styles.push(s.clone());
            }
            let split: Vec<String> = style
                .iter()
                .flat_map(|s| s.split(','))
                .map(str::to_string)
                .collect();
            for requirement in self.requirements.values_mut() {
                if requirement.classes.iter().any(|name| name == id) {
                    copy(&mut self.style_copies, &mut requirement.css_styles, &split)?;
                }
            }
            for element in self.elements.values_mut() {
                if element.classes.iter().any(|name| name == id) {
                    copy(&mut self.style_copies, &mut element.css_styles, &split)?;
                }
            }
        }
        Ok(())
    }

    pub fn set_acc_title(&mut self, text: &str) {
        self.acc_title = Some(js::trim_start(text).to_string());
    }

    pub fn set_acc_description(&mut self, text: &str) {
        self.acc_descr = Some(RE_INDENT.replace(text, |_| "\n".into()));
    }
}
