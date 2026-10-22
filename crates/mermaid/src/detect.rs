//! Which diagram a source is, from its first keyword.

use crate::{string, util::chars};
use serde::{Serialize, Serializer};

/// Every diagram type Mermaid 12.1 registers, by the name Pele gives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Flowchart,
    Sequence,
    Class,
    State,
    Er,
    Pie,
    Gantt,
    GitGraph,
    Mindmap,
    Timeline,
    Journey,
    XyChart,
    QuadrantChart,
    Requirement,
    C4,
    Sankey,
    Block,
    Packet,
    Kanban,
    Architecture,
    Radar,
    Treemap,
    Info,
    Agentflow,
    Usecase,
    Railroad,
    TreeView,
    Venn,
    Cynefin,
    Wardley,
    Swimlane,
    Ishikawa,
    Eventmodeling,
}

impl Kind {
    /// Every variant, in declaration order.
    pub const ALL: [Kind; 33] = [
        Self::Flowchart,
        Self::Sequence,
        Self::Class,
        Self::State,
        Self::Er,
        Self::Pie,
        Self::Gantt,
        Self::GitGraph,
        Self::Mindmap,
        Self::Timeline,
        Self::Journey,
        Self::XyChart,
        Self::QuadrantChart,
        Self::Requirement,
        Self::C4,
        Self::Sankey,
        Self::Block,
        Self::Packet,
        Self::Kanban,
        Self::Architecture,
        Self::Radar,
        Self::Treemap,
        Self::Info,
        Self::Agentflow,
        Self::Usecase,
        Self::Railroad,
        Self::TreeView,
        Self::Venn,
        Self::Cynefin,
        Self::Wardley,
        Self::Swimlane,
        Self::Ishikawa,
        Self::Eventmodeling,
    ];

    /// The name Pele and the SVG's `aria-roledescription` use.
    pub fn name(self) -> &'static str {
        match self {
            Self::Flowchart => "flowchart",
            Self::Sequence => "sequence",
            Self::Class => "class",
            Self::State => "state",
            Self::Er => "er",
            Self::Pie => "pie",
            Self::Gantt => "gantt",
            Self::GitGraph => "gitGraph",
            Self::Mindmap => "mindmap",
            Self::Timeline => "timeline",
            Self::Journey => "journey",
            Self::XyChart => "xychart",
            Self::QuadrantChart => "quadrantChart",
            Self::Requirement => "requirement",
            Self::C4 => "c4",
            Self::Sankey => "sankey",
            Self::Block => "block",
            Self::Packet => "packet",
            Self::Kanban => "kanban",
            Self::Architecture => "architecture",
            Self::Radar => "radar",
            Self::Treemap => "treemap",
            Self::Info => "info",
            Self::Agentflow => "agentflow",
            Self::Usecase => "usecase",
            Self::Railroad => "railroad",
            Self::TreeView => "treeView",
            Self::Venn => "venn",
            Self::Cynefin => "cynefin",
            Self::Wardley => "wardley",
            Self::Swimlane => "swimlane",
            Self::Ishikawa => "ishikawa",
            Self::Eventmodeling => "eventmodeling",
        }
    }
}

impl Serialize for Kind {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name())
    }
}

/// Whether `head` starts with `word`, ending at a `\b`.
fn bounded(head: &str, word: &str) -> bool {
    head.starts_with(word)
        && !head[word.len()..]
            .bytes()
            .next()
            .is_some_and(chars::is_word)
}

/// Whether `head` starts with `word`, given in lower case, in either case.
fn caseless(head: &str, word: &str) -> bool {
    chars::starts_with_caseless(head, 0, word)
}

/// Whether a source, after its leading whitespace, is of a kind.
type Test = fn(&str) -> bool;

/// Mermaid's detectors, in the order Mermaid registers them. The first match
/// wins.
static DETECTORS: [(Kind, Test); 34] = [
    (Kind::Flowchart, |h| h.starts_with("flowchart-elk")),
    (Kind::Mindmap, |h| h.starts_with("mindmap")),
    (Kind::Architecture, |h| h.starts_with("architecture")),
    (Kind::Agentflow, |h| bounded(h, "agentflow-beta")),
    // Only the first name is anchored; the others match anywhere.
    (Kind::C4, |h| {
        h.starts_with("C4Context")
            || ["C4Container", "C4Component", "C4Dynamic", "C4Deployment"]
                .iter()
                .any(|name| h.contains(name))
    }),
    (Kind::Kanban, |h| h.starts_with("kanban")),
    (Kind::Class, |h| h.starts_with("classDiagram")),
    (Kind::Er, |h| h.starts_with("erDiagram")),
    (Kind::Gantt, |h| h.starts_with("gantt")),
    (Kind::Info, |h| h.starts_with("info")),
    (Kind::Pie, |h| h.starts_with("pie")),
    (Kind::Requirement, |h| h.starts_with("requirement")),
    (Kind::Sequence, |h| h.starts_with("sequenceDiagram")),
    (Kind::Swimlane, |h| bounded(h, "swimlane-beta")),
    (Kind::Flowchart, |h| {
        h.starts_with("graph") || h.starts_with("flowchart")
    }),
    (Kind::Timeline, |h| h.starts_with("timeline")),
    (Kind::GitGraph, |h| h.starts_with("gitGraph")),
    (Kind::State, |h| h.starts_with("stateDiagram")),
    (Kind::Journey, |h| h.starts_with("journey")),
    (Kind::QuadrantChart, |h| h.starts_with("quadrantChart")),
    (Kind::Sankey, |h| h.starts_with("sankey")),
    (Kind::Packet, |h| h.starts_with("packet")),
    (Kind::XyChart, |h| h.starts_with("xychart")),
    (Kind::Block, |h| h.starts_with("block")),
    (Kind::Eventmodeling, |h| h.starts_with("eventmodeling")),
    (Kind::TreeView, |h| h.starts_with("treeView-beta")),
    (Kind::Radar, |h| h.starts_with("radar-beta")),
    // `ishikawa(-beta)?\b`: a `-` after the name is a boundary already.
    (Kind::Ishikawa, |h| {
        caseless(h, "ishikawa") && !h[8..].bytes().next().is_some_and(chars::is_word)
    }),
    (Kind::Treemap, |h| h.starts_with("treemap")),
    (Kind::Railroad, |h| {
        caseless(h, "railroad-")
            && ["ebnf-beta", "abnf-beta", "peg-beta", "beta"]
                .iter()
                .any(|rest| caseless(&h[9..], rest))
    }),
    (Kind::Venn, |h| h.starts_with("venn-beta")),
    (Kind::Wardley, |h| caseless(h, "wardley-beta")),
    (Kind::Cynefin, |h| {
        h.strip_prefix("cynefin-beta").is_some_and(|rest| {
            rest.chars()
                .next()
                .is_none_or(|c| c == ':' || chars::is_space(c))
        })
    }),
    (Kind::Usecase, |h| {
        h.strip_prefix("usecase-beta")
            .is_some_and(|rest| rest.chars().next().is_none_or(chars::is_space))
    }),
];

/// The diagram `cleaned` names: source after preprocessing.
pub fn detect(cleaned: &str) -> Option<Kind> {
    let head = string::trim_start(cleaned);
    DETECTORS
        .iter()
        .find(|(_, test)| test(head))
        .map(|(kind, _)| *kind)
}
