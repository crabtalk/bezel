//! Pele's `diagrams/requirement/db.ts`.

use crate::diagrams::common::db::Titles;
use crate::diagrams::common::style_class::StyleClass;
use crate::geom::Direction;
use indexmap::IndexMap;
use serde::Serialize;

use super::types::{
    Relation, Relationship, Requirement, RequirementElement, RequirementType, Risk, VerifyMethod,
    or_empty,
};
use crate::{detect::Kind, error::Error};

/// Applying a class copies its declarations onto each node, so a short text
/// can ask for hundreds of millions of copies. Past this many the diagram is
/// refused.
const MAX_STYLE_COPIES: usize = 1_000_000;

/// Pele's `RequirementDb`, serialized with the fields it keeps between
/// statements as Pele's is.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequirementModel {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub direction: Direction,
    pub requirements: IndexMap<String, Requirement>,
    pub elements: IndexMap<String, RequirementElement>,
    pub relations: Vec<Relation>,
    pub classes: IndexMap<String, StyleClass>,
    #[serde(flatten)]
    pub titles: Titles,
    // The grammar reports a body's fields before the requirement or element
    // they belong to.
    new_id: String,
    new_text: String,
    #[serde(serialize_with = "or_empty")]
    new_risk: Option<Risk>,
    #[serde(serialize_with = "or_empty")]
    new_verify_method: Option<VerifyMethod>,
    new_element_type: String,
    new_doc_ref: String,
    style_copies: usize,
}

impl Default for RequirementModel {
    fn default() -> Self {
        Self {
            kind: "requirement",
            direction: Direction::TopToBottom,
            requirements: IndexMap::new(),
            elements: IndexMap::new(),
            relations: Vec::new(),
            classes: IndexMap::new(),
            titles: Titles::default(),
            new_id: String::new(),
            new_text: String::new(),
            new_risk: None,
            new_verify_method: None,
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
    pub fn add_requirement(&mut self, name: &str, kind: RequirementType) {
        if !self.requirements.contains_key(name) {
            let requirement = Requirement {
                name: name.to_string(),
                kind,
                requirement_id: self.new_id.clone(),
                text: self.new_text.clone(),
                risk: self.new_risk,
                verify_method: self.new_verify_method,
                css_styles: Vec::new(),
                classes: vec!["default".into()],
            };
            self.requirements.insert(name.to_string(), requirement);
        }
        self.new_id.clear();
        self.new_text.clear();
        self.new_risk = None;
        self.new_verify_method = None;
    }

    pub fn set_new_req_id(&mut self, id: &str) {
        self.new_id = id.to_string();
    }

    pub fn set_new_req_text(&mut self, text: &str) {
        self.new_text = text.to_string();
    }

    pub fn set_new_req_risk(&mut self, risk: Risk) {
        self.new_risk = Some(risk);
    }

    pub fn set_new_req_verify_method(&mut self, verify_method: VerifyMethod) {
        self.new_verify_method = Some(verify_method);
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

    pub fn add_relationship(&mut self, kind: Relationship, src: &str, dst: &str) {
        self.relations.push(Relation {
            kind,
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
                .or_insert_with(|| StyleClass::new(id));
            for s in style {
                class.push(s);
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
}
