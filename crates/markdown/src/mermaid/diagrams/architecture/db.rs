//! Pele's `diagrams/architecture/db.ts`.

use indexmap::{IndexMap, IndexSet};
use serde::Serialize;

use super::parser::{ArchitectureAst, Side};
use crate::mermaid::{detect::Kind, error::Error, js::Lazy};

static RE_INDENT: Lazy = Lazy::new(r"\n\s+", "g");
static RE_LEAD: Lazy = Lazy::new(r"^\s+", "g");

/// A service or a junction. A junction has no icon, icon text or title.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchNode {
    pub id: String,
    /// `service` or `junction`.
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(rename = "in", skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

impl ArchNode {
    pub fn is_junction(&self) -> bool {
        self.kind == "junction"
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchGroup {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(rename = "in", skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchEdge {
    pub lhs_id: String,
    pub lhs_dir: Side,
    pub lhs_into: bool,
    pub lhs_group: bool,
    pub rhs_id: String,
    pub rhs_dir: Side,
    pub rhs_into: bool,
    pub rhs_group: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchLayoutHint {
    /// `row` or `column`.
    pub direction: String,
    pub members: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureModel {
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acc_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acc_descr: Option<String>,
    pub nodes: IndexMap<String, ArchNode>,
    pub groups: IndexMap<String, ArchGroup>,
    pub edges: Vec<ArchEdge>,
    pub layout_hints: Vec<ArchLayoutHint>,
    /// What each id names: `node` or `group`.
    registered: IndexMap<String, &'static str>,
}

fn fail(message: String) -> Error {
    Error::semantic(message).of(Kind::Architecture)
}

impl ArchitectureModel {
    pub fn new() -> Self {
        Self {
            kind: "architecture",
            title: None,
            acc_title: None,
            acc_descr: None,
            nodes: IndexMap::new(),
            groups: IndexMap::new(),
            edges: Vec::new(),
            layout_hints: Vec::new(),
            registered: IndexMap::new(),
        }
    }

    fn register(&mut self, kind: &str, id: &str, parent: Option<&str>) -> Result<(), Error> {
        if let Some(other) = self.registered.get(id) {
            return Err(fail(format!(
                "The {kind} id [{id}] is already in use by another {other}"
            )));
        }
        if let Some(parent) = parent {
            if id == parent {
                return Err(fail(format!(
                    "The {kind} [{id}] cannot be placed within itself"
                )));
            }
            match self.registered.get(parent) {
                None => {
                    return Err(fail(format!(
                        "The {kind} [{id}]'s parent does not exist. Please make sure the parent is created before this {kind}"
                    )));
                }
                Some(&"node") => {
                    return Err(fail(format!("The {kind} [{id}]'s parent is not a group")));
                }
                Some(_) => {}
            }
        }
        self.registered.insert(
            id.to_string(),
            if kind == "group" { "group" } else { "node" },
        );
        Ok(())
    }

    pub fn add_service(&mut self, node: ArchNode) -> Result<(), Error> {
        self.register("service", &node.id, node.parent.as_deref())?;
        self.nodes.insert(node.id.clone(), node);
        Ok(())
    }

    pub fn add_junction(&mut self, id: String, parent: Option<String>) -> Result<(), Error> {
        self.register("junction", &id, parent.as_deref())?;
        self.nodes.insert(
            id.clone(),
            ArchNode {
                id,
                kind: "junction",
                icon: None,
                icon_text: None,
                title: None,
                parent,
            },
        );
        Ok(())
    }

    pub fn add_group(&mut self, group: ArchGroup) -> Result<(), Error> {
        self.register("group", &group.id, group.parent.as_deref())?;
        self.groups.insert(group.id.clone(), group);
        Ok(())
    }

    pub fn add_edge(&mut self, edge: ArchEdge) -> Result<(), Error> {
        let ends = [
            ("left", &edge.lhs_id, edge.lhs_group),
            ("right", &edge.rhs_id, edge.rhs_group),
        ];
        for (side, id, _) in ends {
            if !self.registered.contains_key(id.as_str()) {
                return Err(fail(format!(
                    "The {side}-hand id [{id}] does not yet exist. Please create the service/group before declaring an edge to it."
                )));
            }
        }
        // Mermaid lets a group id through the check above and then fails
        // with a TypeError.
        for (side, id, _) in ends {
            if !self.nodes.contains_key(id.as_str()) {
                return Err(fail(format!(
                    "The {side}-hand id [{id}] is a group. Add {{group}} to a service inside it instead."
                )));
            }
        }
        let parent = &self.nodes[edge.lhs_id.as_str()].parent;
        if parent.is_some() && *parent == self.nodes[edge.rhs_id.as_str()].parent {
            for (side, id, crosses) in ends {
                if crosses {
                    return Err(fail(format!(
                        "The {side}-hand id [{id}] is modified to traverse the group boundary, but the edge does not pass through two groups."
                    )));
                }
            }
        }
        self.edges.push(edge);
        Ok(())
    }

    pub fn add_layout_hint(&mut self, hint: ArchLayoutHint) -> Result<(), Error> {
        if hint.members.len() < 2 {
            return Err(fail(format!(
                "An align directive requires at least two members; got {}",
                hint.members.len()
            )));
        }
        let mut seen = IndexSet::new();
        for id in &hint.members {
            if self.registered.get(id.as_str()) != Some(&"node") {
                return Err(fail(format!(
                    "align {} references [{id}], which is not a service or junction",
                    hint.direction
                )));
            }
            if !seen.insert(id.as_str()) {
                return Err(fail(format!(
                    "align {} lists [{id}] more than once",
                    hint.direction
                )));
            }
        }
        self.layout_hints.push(hint);
        Ok(())
    }
}

impl Default for ArchitectureModel {
    fn default() -> Self {
        Self::new()
    }
}

/// Mermaid's populateDb: every group, then services, junctions, edges, and
/// alignments.
pub fn populate(ast: ArchitectureAst, db: &mut ArchitectureModel) -> Result<(), Error> {
    if let Some(text) = ast.acc_descr.filter(|text| !text.is_empty()) {
        db.acc_descr = Some(RE_INDENT.replace(&text, |_| "\n".into()));
    }
    if let Some(text) = ast.acc_title.filter(|text| !text.is_empty()) {
        db.acc_title = Some(RE_LEAD.replace(&text, |_| String::new()));
    }
    if let Some(title) = ast.title.filter(|text| !text.is_empty()) {
        db.title = Some(title);
    }
    for group in ast.groups {
        db.add_group(ArchGroup {
            id: group.id,
            icon: group.icon,
            title: group.title,
            parent: group.r#in,
        })?;
    }
    for service in ast.services {
        db.add_service(ArchNode {
            id: service.id,
            kind: "service",
            icon: service.icon,
            icon_text: service.icon_text,
            title: service.title,
            parent: service.r#in,
        })?;
    }
    for junction in ast.junctions {
        db.add_junction(junction.id, junction.r#in)?;
    }
    for edge in ast.edges {
        db.add_edge(ArchEdge {
            lhs_id: edge.lhs_id,
            lhs_dir: edge.lhs_dir,
            lhs_into: edge.lhs_into,
            lhs_group: edge.lhs_group,
            rhs_id: edge.rhs_id,
            rhs_dir: edge.rhs_dir,
            rhs_into: edge.rhs_into,
            rhs_group: edge.rhs_group,
            title: edge.title,
        })?;
    }
    for alignment in ast.alignments {
        db.add_layout_hint(ArchLayoutHint {
            direction: alignment.direction,
            members: alignment.members,
        })?;
    }
    Ok(())
}
