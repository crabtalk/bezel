//! Pele's `diagrams/architecture/db.ts`.

use crate::diagrams::common::db::Titles;
use indexmap::{IndexMap, IndexSet};
use serde::Serialize;

use super::parser::{Alignment, ArchitectureAst};
use crate::{detect::Kind, error::Error, geom::Side};

/// A service or a junction. A junction has no icon, icon text or title.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchNode {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: ArchNodeKind,
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
        self.kind == ArchNodeKind::Junction
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ArchNodeKind {
    Service,
    Junction,
}

/// What an id names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Registered {
    Node,
    Group,
}

impl Registered {
    fn name(self) -> &'static str {
        match self {
            Self::Node => "node",
            Self::Group => "group",
        }
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
    #[serde(serialize_with = "super::parser::side_letter")]
    pub lhs_dir: Side,
    pub lhs_into: bool,
    pub lhs_group: bool,
    pub rhs_id: String,
    #[serde(serialize_with = "super::parser::side_letter")]
    pub rhs_dir: Side,
    pub rhs_into: bool,
    pub rhs_group: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchLayoutHint {
    pub direction: Alignment,
    pub members: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureModel {
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(flatten)]
    pub titles: Titles,
    pub nodes: IndexMap<String, ArchNode>,
    pub groups: IndexMap<String, ArchGroup>,
    pub edges: Vec<ArchEdge>,
    pub layout_hints: Vec<ArchLayoutHint>,
    registered: IndexMap<String, Registered>,
}

impl ArchitectureModel {
    pub fn new() -> Self {
        Self {
            kind: "architecture",
            titles: Titles::default(),
            nodes: IndexMap::new(),
            groups: IndexMap::new(),
            edges: Vec::new(),
            layout_hints: Vec::new(),
            registered: IndexMap::new(),
        }
    }

    /// Registers `id` as `what`; `kind` names it in messages.
    fn register(
        &mut self,
        kind: &str,
        what: Registered,
        id: &str,
        parent: Option<&str>,
    ) -> Result<(), Error> {
        if let Some(other) = self.registered.get(id) {
            return Err(Error::semantic(format!(
                "The {kind} id [{id}] is already in use by another {}",
                other.name()
            ))
            .of(Kind::Architecture));
        }
        if let Some(parent) = parent {
            if id == parent {
                return Err(Error::semantic(format!(
                    "The {kind} [{id}] cannot be placed within itself"
                ))
                .of(Kind::Architecture));
            }
            match self.registered.get(parent) {
                None => {
                    return Err(Error::semantic(format!(
                        "The {kind} [{id}]'s parent does not exist. Please make sure the parent is created before this {kind}"
                    )).of(Kind::Architecture));
                }
                Some(Registered::Node) => {
                    return Err(Error::semantic(format!(
                        "The {kind} [{id}]'s parent is not a group"
                    ))
                    .of(Kind::Architecture));
                }
                Some(_) => {}
            }
        }
        self.registered.insert(id.to_string(), what);
        Ok(())
    }

    pub fn add_service(&mut self, node: ArchNode) -> Result<(), Error> {
        self.register(
            "service",
            Registered::Node,
            &node.id,
            node.parent.as_deref(),
        )?;
        self.nodes.insert(node.id.clone(), node);
        Ok(())
    }

    pub fn add_junction(&mut self, id: String, parent: Option<String>) -> Result<(), Error> {
        self.register("junction", Registered::Node, &id, parent.as_deref())?;
        self.nodes.insert(
            id.clone(),
            ArchNode {
                id,
                kind: ArchNodeKind::Junction,
                icon: None,
                icon_text: None,
                title: None,
                parent,
            },
        );
        Ok(())
    }

    pub fn add_group(&mut self, group: ArchGroup) -> Result<(), Error> {
        self.register(
            "group",
            Registered::Group,
            &group.id,
            group.parent.as_deref(),
        )?;
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
                return Err(Error::semantic(format!(
                    "The {side}-hand id [{id}] does not yet exist. Please create the service/group before declaring an edge to it."
                )).of(Kind::Architecture));
            }
        }
        // Mermaid lets a group id through the check above and then fails
        // with a TypeError.
        for (side, id, _) in ends {
            if !self.nodes.contains_key(id.as_str()) {
                return Err(Error::semantic(format!(
                    "The {side}-hand id [{id}] is a group. Add {{group}} to a service inside it instead."
                )).of(Kind::Architecture));
            }
        }
        let parent = &self.nodes[edge.lhs_id.as_str()].parent;
        if parent.is_some() && *parent == self.nodes[edge.rhs_id.as_str()].parent {
            for (side, id, crosses) in ends {
                if crosses {
                    return Err(Error::semantic(format!(
                        "The {side}-hand id [{id}] is modified to traverse the group boundary, but the edge does not pass through two groups."
                    )).of(Kind::Architecture));
                }
            }
        }
        self.edges.push(edge);
        Ok(())
    }

    pub fn add_layout_hint(&mut self, hint: ArchLayoutHint) -> Result<(), Error> {
        if hint.members.len() < 2 {
            return Err(Error::semantic(format!(
                "An align directive requires at least two members; got {}",
                hint.members.len()
            ))
            .of(Kind::Architecture));
        }
        let mut seen = IndexSet::new();
        for id in &hint.members {
            if self.registered.get(id.as_str()) != Some(&Registered::Node) {
                return Err(Error::semantic(format!(
                    "align {} references [{id}], which is not a service or junction",
                    hint.direction.name()
                ))
                .of(Kind::Architecture));
            }
            if !seen.insert(id.as_str()) {
                return Err(Error::semantic(format!(
                    "align {} lists [{id}] more than once",
                    hint.direction.name()
                ))
                .of(Kind::Architecture));
            }
        }
        self.layout_hints.push(hint);
        Ok(())
    }

    /// Mermaid's populateDb, with the front matter `title`: every group,
    /// then services, junctions, edges, and alignments.
    pub fn populate(&mut self, ast: ArchitectureAst, title: Option<&str>) -> Result<(), Error> {
        self.titles = ast.header.titles(title.filter(|title| !title.is_empty()));
        for group in ast.groups {
            self.add_group(ArchGroup {
                id: group.id,
                icon: group.icon,
                title: group.title,
                parent: group.r#in,
            })?;
        }
        for service in ast.services {
            self.add_service(ArchNode {
                id: service.id,
                kind: ArchNodeKind::Service,
                icon: service.icon,
                icon_text: service.icon_text,
                title: service.title,
                parent: service.r#in,
            })?;
        }
        for junction in ast.junctions {
            self.add_junction(junction.id, junction.r#in)?;
        }
        for edge in ast.edges {
            self.add_edge(ArchEdge {
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
            self.add_layout_hint(ArchLayoutHint {
                direction: alignment.direction,
                members: alignment.members,
            })?;
        }
        Ok(())
    }
}

impl Default for ArchitectureModel {
    fn default() -> Self {
        Self::new()
    }
}
