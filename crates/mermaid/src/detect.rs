//! Which diagram a source is, from its first keyword.

use serde::Serialize;

use super::js::Lazy;

/// Every diagram type Mermaid 12.1 registers, by the name Pele gives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
pub enum Kind {
    #[serde(rename = "flowchart")]
    Flowchart,
    #[serde(rename = "sequence")]
    Sequence,
    #[serde(rename = "class")]
    Class,
    #[serde(rename = "state")]
    State,
    #[serde(rename = "er")]
    Er,
    #[serde(rename = "pie")]
    Pie,
    #[serde(rename = "gantt")]
    Gantt,
    #[serde(rename = "gitGraph")]
    GitGraph,
    #[serde(rename = "mindmap")]
    Mindmap,
    #[serde(rename = "timeline")]
    Timeline,
    #[serde(rename = "journey")]
    Journey,
    #[serde(rename = "xychart")]
    XyChart,
    #[serde(rename = "quadrantChart")]
    QuadrantChart,
    #[serde(rename = "requirement")]
    Requirement,
    #[serde(rename = "c4")]
    C4,
    #[serde(rename = "sankey")]
    Sankey,
    #[serde(rename = "block")]
    Block,
    #[serde(rename = "packet")]
    Packet,
    #[serde(rename = "kanban")]
    Kanban,
    #[serde(rename = "architecture")]
    Architecture,
    #[serde(rename = "radar")]
    Radar,
    #[serde(rename = "treemap")]
    Treemap,
    #[serde(rename = "info")]
    Info,
    #[serde(rename = "agentflow")]
    Agentflow,
    #[serde(rename = "usecase")]
    Usecase,
    #[serde(rename = "railroad")]
    Railroad,
    #[serde(rename = "treeView")]
    TreeView,
    #[serde(rename = "venn")]
    Venn,
    #[serde(rename = "cynefin")]
    Cynefin,
    #[serde(rename = "wardley")]
    Wardley,
    #[serde(rename = "swimlane")]
    Swimlane,
    #[serde(rename = "ishikawa")]
    Ishikawa,
    #[serde(rename = "eventmodeling")]
    Eventmodeling,
}

impl Kind {
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

/// Mermaid's detectors, in the order Mermaid registers them. The first match
/// wins.
static DETECTORS: [(Kind, Lazy); 34] = [
    (Kind::Flowchart, Lazy::new(r"^\s*flowchart-elk", "")),
    (Kind::Mindmap, Lazy::new(r"^\s*mindmap", "")),
    (Kind::Architecture, Lazy::new(r"^\s*architecture", "")),
    (Kind::Agentflow, Lazy::new(r"^\s*agentflow-beta\b", "")),
    (
        Kind::C4,
        Lazy::new(
            r"^\s*C4Context|C4Container|C4Component|C4Dynamic|C4Deployment",
            "",
        ),
    ),
    (Kind::Kanban, Lazy::new(r"^\s*kanban", "")),
    (Kind::Class, Lazy::new(r"^\s*classDiagram", "")),
    (Kind::Er, Lazy::new(r"^\s*erDiagram", "")),
    (Kind::Gantt, Lazy::new(r"^\s*gantt", "")),
    (Kind::Info, Lazy::new(r"^\s*info", "")),
    (Kind::Pie, Lazy::new(r"^\s*pie", "")),
    (
        Kind::Requirement,
        Lazy::new(r"^\s*requirement(Diagram)?", ""),
    ),
    (Kind::Sequence, Lazy::new(r"^\s*sequenceDiagram", "")),
    (Kind::Swimlane, Lazy::new(r"^\s*swimlane-beta\b", "")),
    (Kind::Flowchart, Lazy::new(r"^\s*(graph|flowchart)", "")),
    (Kind::Timeline, Lazy::new(r"^\s*timeline", "")),
    (Kind::GitGraph, Lazy::new(r"^\s*gitGraph", "")),
    (Kind::State, Lazy::new(r"^\s*stateDiagram", "")),
    (Kind::Journey, Lazy::new(r"^\s*journey", "")),
    (Kind::QuadrantChart, Lazy::new(r"^\s*quadrantChart", "")),
    (Kind::Sankey, Lazy::new(r"^\s*sankey(-beta)?", "")),
    (Kind::Packet, Lazy::new(r"^\s*packet(-beta)?", "")),
    (Kind::XyChart, Lazy::new(r"^\s*xychart(-beta)?", "")),
    (Kind::Block, Lazy::new(r"^\s*block(-beta)?", "")),
    (Kind::Eventmodeling, Lazy::new(r"^\s*eventmodeling", "")),
    (Kind::TreeView, Lazy::new(r"^\s*treeView-beta", "")),
    (Kind::Radar, Lazy::new(r"^\s*radar-beta", "")),
    (Kind::Ishikawa, Lazy::new(r"^\s*ishikawa(-beta)?\b", "i")),
    (Kind::Treemap, Lazy::new(r"^\s*treemap", "")),
    (
        Kind::Railroad,
        Lazy::new(r"^\s*railroad-(?:ebnf-|abnf-|peg-)?beta", "i"),
    ),
    (Kind::Venn, Lazy::new(r"^\s*venn-beta", "")),
    (Kind::Wardley, Lazy::new(r"^\s*wardley-beta", "i")),
    (Kind::Cynefin, Lazy::new(r"^\s*cynefin-beta(?:[\s:]|$)", "")),
    (Kind::Usecase, Lazy::new(r"^\s*usecase-beta(?:\s|$)", "")),
];

/// The diagram `cleaned` names: source after preprocessing.
pub fn detect(cleaned: &str) -> Option<Kind> {
    DETECTORS
        .iter()
        .find(|(_, re)| re.test(cleaned))
        .map(|(kind, _)| *kind)
}
