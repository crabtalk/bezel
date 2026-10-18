//! Pele's `diagrams/git/db.ts`.

use indexmap::IndexMap;
use serde::Serialize;

use super::parser::{CommitTypeName, GitGraphAst, StatementAst};
use crate::mermaid::{Config, Error, Kind, Value, js::Lazy};

pub mod commit_type {
    pub const NORMAL: u8 = 0;
    pub const REVERSE: u8 = 1;
    pub const HIGHLIGHT: u8 = 2;
    pub const MERGE: u8 = 3;
    pub const CHERRY_PICK: u8 = 4;
}

static RE_LEAD: Lazy = Lazy::new(r"^\s+", "g");
static RE_INDENT: Lazy = Lazy::new(r"\n\s+", "g");

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommit {
    pub id: String,
    pub message: String,
    pub seq: usize,
    #[serde(rename = "type")]
    pub kind: u8,
    pub tags: Vec<String>,
    pub parents: Vec<String>,
    pub branch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_type: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_id: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitModel {
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acc_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acc_descr: Option<String>,
    pub direction: &'static str,
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

fn fail(message: impl Into<String>) -> Error {
    Error::semantic(message).of(Kind::GitGraph)
}

fn type_code(name: CommitTypeName) -> u8 {
    match name {
        CommitTypeName::Reverse => commit_type::REVERSE,
        CommitTypeName::Highlight => commit_type::HIGHLIGHT,
        CommitTypeName::Normal => commit_type::NORMAL,
    }
}

/// What Mermaid's gitGraph database holds while statements are read.
struct Db {
    commits: IndexMap<String, GitCommit>,
    branches: IndexMap<String, Option<String>>,
    orders: IndexMap<String, Option<f64>>,
    warnings: Vec<String>,
    head: Option<Box<GitCommit>>,
    current: String,
    direction: &'static str,
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
            return Err(fail(format!(
                "Trying to checkout branch which is not yet created. (Help try using \"branch {branch}\")"
            )));
        };
        self.head = id
            .as_ref()
            .filter(|id| !id.is_empty())
            .and_then(|id| self.commits.get(id).cloned().map(Box::new));
        self.current = branch.to_string();
        Ok(())
    }

    fn commit(&mut self, msg: String, id: Option<String>, kind: u8, tags: Vec<String>) {
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
            return Err(fail(format!(
                "Trying to create an existing branch. (Help: Either use a new name if you want create a new branch or try using \"checkout {name}\")"
            )));
        }
        self.branches.insert(name.clone(), self.head_id());
        self.orders.insert(name.clone(), order);
        self.checkout(&name)
    }

    fn merge(
        &mut self,
        other: &str,
        custom_id: String,
        kind: Option<u8>,
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
            return Err(fail(format!("Cannot merge branch '{other}' into itself.")));
        }
        if current == other {
            return Err(fail(
                "Incorrect usage of \"merge\". Cannot merge a branch to itself",
            ));
        }
        let Some(current_commit) = current_commit else {
            return Err(fail(format!(
                "Incorrect usage of \"merge\". Current branch ({current})has no commits"
            )));
        };
        if !self.branches.contains_key(other) {
            return Err(fail(format!(
                "Incorrect usage of \"merge\". Branch to be merged ({other}) does not exist"
            )));
        }
        let Some(other_commit) = other_commit else {
            return Err(fail(format!(
                "Incorrect usage of \"merge\". Branch to be merged ({other}) has no commits"
            )));
        };
        if current_commit.id == other_commit.id {
            return Err(fail(
                "Incorrect usage of \"merge\". Both branches have same head",
            ));
        }
        if !custom_id.is_empty() && self.commits.contains_key(&custom_id) {
            return Err(fail(format!(
                "Incorrect usage of \"merge\". Commit with id:{custom_id} already exists, use different custom id"
            )));
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
            kind: commit_type::MERGE,
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
            return Err(fail(
                "Incorrect usage of \"cherryPick\". Source commit id should exist and provided",
            ));
        };
        let parent = parent.filter(|parent| !parent.is_empty());
        if let Some(parent) = &parent
            && !source.parents.contains(parent)
        {
            return Err(fail(
                "Invalid operation: The specified parent commit is not an immediate parent of the cherry-picked commit.",
            ));
        }
        if source.kind == commit_type::MERGE && parent.is_none() {
            return Err(fail(
                "Incorrect usage of cherry-pick: If the source commit is a merge commit, an immediate parent commit must be specified.",
            ));
        }
        if !target_id.is_empty() && self.commits.contains_key(target_id) {
            return Ok(());
        }
        let current = self.current.clone();
        if source.branch == current {
            return Err(fail(
                "Incorrect usage of \"cherryPick\". Source commit is already on current branch",
            ));
        }
        let current_head = self.branches.get(&current).cloned().flatten();
        if !current_head
            .as_ref()
            .is_some_and(|id| !id.is_empty() && self.commits.contains_key(id))
        {
            return Err(fail(format!(
                "Incorrect usage of \"cherry-pick\". Current branch ({current})has no commits"
            )));
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
                if source.kind == commit_type::MERGE {
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
            kind: commit_type::CHERRY_PICK,
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
                    kind.map_or(commit_type::NORMAL, type_code),
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
            } => self.merge(&branch, id.unwrap_or_default(), kind.map(type_code), tags),
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

pub fn build_git(
    ast: GitGraphAst,
    config: &Config,
    title: Option<&str>,
) -> Result<GitModel, Error> {
    let git = config.section("gitGraph");
    let main_name = git
        .and_then(|git| git.get("mainBranchName"))
        .and_then(Value::as_str)
        .unwrap_or("main")
        .to_string();
    let main_order = match git.and_then(|git| git.get("mainBranchOrder")) {
        Some(Value::Number(order)) => *order,
        _ => 0.0,
    };

    let mut db = Db {
        commits: IndexMap::new(),
        branches: IndexMap::from([(main_name.clone(), None)]),
        orders: IndexMap::from([(main_name.clone(), Some(main_order))]),
        warnings: Vec::new(),
        head: None,
        current: main_name,
        direction: "LR",
        seq: 0,
    };

    if let Some(dir) = ast.dir {
        db.direction = dir;
    }
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
                order.unwrap_or_else(|| crate::mermaid::js::parse_float(&format!("0.{i}"))),
            )
        })
        .collect();
    lanes.sort_by(|a, b| {
        (a.1 - b.1)
            .partial_cmp(&0.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(GitModel {
        kind: "gitGraph",
        title: ast
            .title
            .filter(|title| !title.is_empty())
            .or_else(|| title.map(str::to_string)),
        acc_title: ast
            .acc_title
            .filter(|text| !text.is_empty())
            .map(|text| RE_LEAD.replace(&text, |_| String::new())),
        acc_descr: ast
            .acc_descr
            .filter(|text| !text.is_empty())
            .map(|text| RE_INDENT.replace(&text, |_| "\n".into())),
        direction: db.direction,
        commits: db.commits,
        branches: db.branches,
        lanes: lanes.into_iter().map(|(name, _)| name).collect(),
        current_branch: db.current,
        head: db.head,
        warnings: db.warnings,
    })
}
