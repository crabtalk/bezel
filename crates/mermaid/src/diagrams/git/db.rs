//! Pele's `diagrams/git/db.ts`.

use super::GitGraphConfig;
use crate::diagrams::common::db::Titles;
use indexmap::IndexMap;
use serde::Serialize;
use serde_repr::Serialize_repr;

use super::parser::{GitGraphAst, StatementAst};
use crate::{Error, Kind, geom::Direction, js};

/// A commit's kind, numbered as in Mermaid.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize_repr)]
#[repr(u8)]
pub enum CommitType {
    #[default]
    Normal = 0,
    Reverse = 1,
    Highlight = 2,
    Merge = 3,
    CherryPick = 4,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommit {
    pub id: String,
    pub message: String,
    pub seq: usize,
    #[serde(rename = "type")]
    pub kind: CommitType,
    pub tags: Vec<String>,
    pub parents: Vec<String>,
    pub branch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_type: Option<CommitType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_id: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitModel {
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(flatten)]
    pub titles: Titles,
    pub direction: Direction,
    /// Keyed by commit id, in the order the ids first appeared.
    pub commits: IndexMap<String, GitCommit>,
    /// The id of each branch's latest commit, or `None` while the branch has
    /// none.
    pub branches: IndexMap<String, Option<String>>,
    /// Branch names in drawing order: by `order`, then by creation.
    pub lanes: Vec<String>,
    pub current_branch: String,
    pub head: Option<Box<GitCommit>>,
    pub warnings: Vec<String>,
}

/// What Mermaid's gitGraph database holds while statements are read.
struct Db {
    commits: IndexMap<String, GitCommit>,
    branches: IndexMap<String, Option<String>>,
    orders: IndexMap<String, Option<f64>>,
    warnings: Vec<String>,
    head: Option<Box<GitCommit>>,
    current: String,
    direction: Direction,
    seq: usize,
}

impl Db {
    /// Mermaid appends seven random hex digits to the sequence number. These
    /// digits are derived from the sequence number instead, so the same text
    /// always gives the same ids.
    fn next_id(&self) -> String {
        let mut salt = self.seq as u64 + 1;
        loop {
            let mixed = (salt as u32).wrapping_mul(0x9e37_79b1);
            let id = format!("{}-{}", self.seq, &format!("{mixed:08x}")[..7]);
            if !self.commits.contains_key(&id) {
                return id;
            }
            salt += 0x9e37_79b1;
        }
    }

    fn add(&mut self, commit: GitCommit) {
        self.head = Some(Box::new(commit.clone()));
        self.branches
            .insert(self.current.clone(), Some(commit.id.clone()));
        self.commits.insert(commit.id.clone(), commit);
    }

    fn head_id(&self) -> Option<String> {
        self.head.as_ref().map(|head| head.id.clone())
    }

    fn checkout(&mut self, branch: &str) -> Result<(), Error> {
        let Some(id) = self.branches.get(branch) else {
            return Err(Error::semantic(format!(
                "Trying to checkout branch which is not yet created. (Help try using \"branch {branch}\")"
            )).of(Kind::GitGraph));
        };
        self.head = id
            .as_ref()
            .filter(|id| !id.is_empty())
            .and_then(|id| self.commits.get(id).cloned().map(Box::new));
        self.current = branch.to_string();
        Ok(())
    }

    fn commit(&mut self, msg: String, id: Option<String>, kind: CommitType, tags: Vec<String>) {
        let commit = GitCommit {
            id: id
                .filter(|id| !id.is_empty())
                .unwrap_or_else(|| self.next_id()),
            message: msg,
            seq: self.seq,
            kind,
            tags,
            parents: self.head_id().into_iter().collect(),
            branch: self.current.clone(),
            custom_type: None,
            custom_id: None,
        };
        self.seq += 1;
        if self.commits.contains_key(&commit.id) {
            self.warnings
                .push(format!("Commit ID {} already exists", commit.id));
        }
        self.add(commit);
    }

    fn branch(&mut self, name: String, order: Option<f64>) -> Result<(), Error> {
        if self.branches.contains_key(&name) {
            return Err(Error::semantic(format!(
                "Trying to create an existing branch. (Help: Either use a new name if you want create a new branch or try using \"checkout {name}\")"
            )).of(Kind::GitGraph));
        }
        self.branches.insert(name.clone(), self.head_id());
        self.orders.insert(name.clone(), order);
        self.checkout(&name)
    }

    fn merge(
        &mut self,
        other: &str,
        custom_id: String,
        kind: Option<CommitType>,
        tags: Vec<String>,
    ) -> Result<(), Error> {
        let current = self.current.clone();
        let current_head = self.branches.get(&current).cloned().flatten();
        let other_head = self.branches.get(other).cloned().flatten();
        let current_commit = current_head
            .as_ref()
            .filter(|id| !id.is_empty())
            .and_then(|id| self.commits.get(id));
        let other_commit = other_head
            .as_ref()
            .filter(|id| !id.is_empty())
            .and_then(|id| self.commits.get(id));
        if let (Some(current_commit), Some(_)) = (current_commit, other_commit)
            && current_commit.branch == other
        {
            return Err(
                Error::semantic(format!("Cannot merge branch '{other}' into itself."))
                    .of(Kind::GitGraph),
            );
        }
        if current == other {
            return Err(Error::semantic(
                "Incorrect usage of \"merge\". Cannot merge a branch to itself",
            )
            .of(Kind::GitGraph));
        }
        let Some(current_commit) = current_commit else {
            return Err(Error::semantic(format!(
                "Incorrect usage of \"merge\". Current branch ({current})has no commits"
            ))
            .of(Kind::GitGraph));
        };
        if !self.branches.contains_key(other) {
            return Err(Error::semantic(format!(
                "Incorrect usage of \"merge\". Branch to be merged ({other}) does not exist"
            ))
            .of(Kind::GitGraph));
        }
        let Some(other_commit) = other_commit else {
            return Err(Error::semantic(format!(
                "Incorrect usage of \"merge\". Branch to be merged ({other}) has no commits"
            ))
            .of(Kind::GitGraph));
        };
        if current_commit.id == other_commit.id {
            return Err(Error::semantic(
                "Incorrect usage of \"merge\". Both branches have same head",
            )
            .of(Kind::GitGraph));
        }
        if !custom_id.is_empty() && self.commits.contains_key(&custom_id) {
            return Err(Error::semantic(format!(
                "Incorrect usage of \"merge\". Commit with id:{custom_id} already exists, use different custom id"
            )).of(Kind::GitGraph));
        }
        let parents = match self.head_id() {
            None => Vec::new(),
            Some(head) => vec![head, other_head.unwrap_or_default()],
        };
        let commit = GitCommit {
            id: if custom_id.is_empty() {
                self.next_id()
            } else {
                custom_id.clone()
            },
            message: format!("merged branch {other} into {current}"),
            seq: self.seq,
            kind: CommitType::Merge,
            tags,
            parents,
            branch: current,
            custom_type: kind,
            custom_id: Some(!custom_id.is_empty()),
        };
        self.seq += 1;
        self.add(commit);
        Ok(())
    }

    fn cherry_pick(
        &mut self,
        source_id: Option<String>,
        target_id: &str,
        tags: Option<Vec<String>>,
        parent: Option<String>,
    ) -> Result<(), Error> {
        let Some(source) = source_id
            .as_ref()
            .filter(|id| !id.is_empty())
            .and_then(|id| self.commits.get(id))
            .cloned()
        else {
            return Err(Error::semantic(
                "Incorrect usage of \"cherryPick\". Source commit id should exist and provided",
            )
            .of(Kind::GitGraph));
        };
        let parent = parent.filter(|parent| !parent.is_empty());
        if let Some(parent) = &parent
            && !source.parents.contains(parent)
        {
            return Err(Error::semantic(
                "Invalid operation: The specified parent commit is not an immediate parent of the cherry-picked commit.",
            ).of(Kind::GitGraph));
        }
        if source.kind == CommitType::Merge && parent.is_none() {
            return Err(Error::semantic(
                "Incorrect usage of cherry-pick: If the source commit is a merge commit, an immediate parent commit must be specified.",
            ).of(Kind::GitGraph));
        }
        if !target_id.is_empty() && self.commits.contains_key(target_id) {
            return Ok(());
        }
        let current = self.current.clone();
        if source.branch == current {
            return Err(Error::semantic(
                "Incorrect usage of \"cherryPick\". Source commit is already on current branch",
            )
            .of(Kind::GitGraph));
        }
        let current_head = self.branches.get(&current).cloned().flatten();
        if !current_head
            .as_ref()
            .is_some_and(|id| !id.is_empty() && self.commits.contains_key(id))
        {
            return Err(Error::semantic(format!(
                "Incorrect usage of \"cherry-pick\". Current branch ({current})has no commits"
            ))
            .of(Kind::GitGraph));
        }
        let parents = match self.head_id() {
            None => Vec::new(),
            Some(head) => vec![head, source.id.clone()],
        };
        let tags = match tags {
            Some(tags) => tags.into_iter().filter(|tag| !tag.is_empty()).collect(),
            None => vec![format!(
                "cherry-pick:{}{}",
                source.id,
                if source.kind == CommitType::Merge {
                    format!("|parent:{}", parent.as_deref().unwrap_or("undefined"))
                } else {
                    String::new()
                }
            )],
        };
        let commit = GitCommit {
            id: self.next_id(),
            message: format!("cherry-picked {} into {current}", source.message),
            seq: self.seq,
            kind: CommitType::CherryPick,
            tags,
            parents,
            branch: current,
            custom_type: None,
            custom_id: None,
        };
        self.seq += 1;
        self.add(commit);
        Ok(())
    }

    fn statement(&mut self, statement: StatementAst) -> Result<(), Error> {
        match statement {
            StatementAst::Commit {
                id,
                message,
                tags,
                kind,
            } => {
                self.commit(
                    message.unwrap_or_default(),
                    id,
                    kind.unwrap_or_default(),
                    tags,
                );
                Ok(())
            }
            StatementAst::Branch { name, order } => self.branch(name, Some(order.unwrap_or(0.0))),
            StatementAst::Merge {
                branch,
                id,
                tags,
                kind,
            } => self.merge(&branch, id.unwrap_or_default(), kind, tags),
            StatementAst::Checkout { branch } => self.checkout(&branch),
            StatementAst::CherryPicking { id, tags, parent } => self.cherry_pick(
                id,
                "",
                if tags.is_empty() { None } else { Some(tags) },
                parent,
            ),
        }
    }
}

impl GitModel {
    pub fn new(
        ast: GitGraphAst,
        config: &GitGraphConfig,
        title: Option<&str>,
    ) -> Result<Self, Error> {
        let main_name = config
            .main_branch_name
            .as_deref()
            .unwrap_or("main")
            .to_string();
        let main_order = config.main_branch_order.unwrap_or(0.0);

        let mut db = Db {
            commits: IndexMap::new(),
            branches: IndexMap::from([(main_name.clone(), None)]),
            orders: IndexMap::from([(main_name.clone(), Some(main_order))]),
            warnings: Vec::new(),
            head: None,
            current: main_name,
            direction: ast.dir.unwrap_or(Direction::LeftToRight),
            seq: 0,
        };

        for statement in ast.statements {
            db.statement(statement)?;
        }

        // A branch made without an order sorts by creation among those of order
        // zero, as in Mermaid.
        let mut lanes: Vec<(String, f64)> = db
            .orders
            .iter()
            .enumerate()
            .map(|(i, (name, order))| {
                (
                    name.clone(),
                    order.unwrap_or_else(|| format!("0.{i}").parse().unwrap_or_default()),
                )
            })
            .collect();
        lanes.sort_by(|a, b| js::compare(a.1 - b.1));

        Ok(Self {
            kind: "gitGraph",
            titles: ast.header.titles(title),
            direction: db.direction,
            commits: db.commits,
            branches: db.branches,
            lanes: lanes.into_iter().map(|(name, _)| name).collect(),
            current_branch: db.current,
            head: db.head,
            warnings: db.warnings,
        })
    }
}
