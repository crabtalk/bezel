//! Every diagram type, and the one a source names.

use serde::Serialize;

use super::{Config, Error, Kind, Options, ParseLimits, Rendered};

pub mod architecture;
pub mod block;
pub mod c4;
pub mod class;
pub mod common;
pub mod er;
pub mod flowchart;
pub mod gantt;
pub mod git;
pub mod info;
pub mod ishikawa;
pub mod journey;
pub mod kanban;
pub mod mindmap;
pub mod packet;
pub mod pie;
pub mod quadrant;
pub mod radar;
pub mod railroad;
pub mod requirement;
pub mod sankey;
pub mod sequence;
pub mod state;
pub mod timeline;
pub mod treemap;
pub mod treeview;
pub mod venn;
pub mod xychart;

/// What a diagram type provides. `source` has been through preprocessing and
/// entity encoding and ends with a newline, as Mermaid's parsers expect.
pub trait Diagram: Sync {
    /// The name of this type's section in Mermaid's config, when it is not
    /// the type's own name.
    fn section(&self) -> Option<&'static str> {
        None
    }

    /// A type that reports source positions is given the text with its `%%`
    /// comment lines kept. `line_offset` counts the front matter lines
    /// removed before it.
    fn keep_comments(&self) -> bool {
        false
    }

    fn parse(
        &self,
        source: &str,
        config: &Config,
        title: Option<&str>,
        line_offset: usize,
        limits: &ParseLimits,
    ) -> Result<Model, Error>;

    fn render(&self, model: &Model, config: &Config, options: &Options) -> Result<Rendered, Error>;
}

/// A parsed diagram, of whichever type, as Pele's `parse` answers it.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
pub enum Model {
    GitGraph(git::GitModel),
    Gantt(Box<gantt::GanttDb>),
    Info(info::InfoModel),
    Flowchart(Box<flowchart::FlowDb>),
    XyChart(xychart::XyChartModel),
    Quadrant(quadrant::QuadrantModel),
    Sankey(sankey::SankeyModel),
    Treemap(treemap::TreemapModel),
    Radar(radar::RadarModel),
    Pie(pie::PieModel),
    Mindmap(mindmap::MindmapModel),
    Packet(packet::PacketModel),
    TreeView(treeview::TreeViewModel),
    Ishikawa(ishikawa::db::IshikawaDb),
    Railroad(railroad::db::RailroadDb),
    Venn(venn::db::VennDb),
    Journey(journey::JourneyModel),
    Kanban(kanban::KanbanModel),
    Timeline(timeline::TimelineModel),
    Class(Box<class::ClassModel>),
    State(Box<state::StateModel>),
    Er(Box<er::ErModel>),
    Requirement(Box<requirement::RequirementModel>),
    Sequence(Box<sequence::SequenceModel>),
    Architecture(Box<architecture::ArchitectureModel>),
    Block(Box<block::BlockModel>),
    C4(c4::C4Model),
}

/// The type `kind`, when this build has it.
pub fn diagram(kind: Kind) -> Option<&'static dyn Diagram> {
    Some(match kind {
        Kind::GitGraph => &git::GitGraph,
        Kind::Gantt => &gantt::Gantt,
        Kind::Info => &info::Info,
        Kind::Flowchart => &flowchart::Flowchart,
        Kind::XyChart => &xychart::XyChart,
        Kind::QuadrantChart => &quadrant::Quadrant,
        Kind::Sankey => &sankey::Sankey,
        Kind::Treemap => &treemap::Treemap,
        Kind::Radar => &radar::Radar,
        Kind::Pie => &pie::Pie,
        Kind::Mindmap => &mindmap::Mindmap,
        Kind::Packet => &packet::Packet,
        Kind::TreeView => &treeview::TreeView,
        Kind::Ishikawa => &ishikawa::Ishikawa,
        Kind::Railroad => &railroad::Railroad,
        Kind::Venn => &venn::Venn,
        Kind::Journey => &journey::Journey,
        Kind::Kanban => &kanban::Kanban,
        Kind::Timeline => &timeline::Timeline,
        Kind::Class => &class::Class,
        Kind::State => &state::State,
        Kind::Er => &er::Er,
        Kind::Requirement => &requirement::Requirement,
        Kind::Sequence => &sequence::Sequence,
        Kind::Architecture => &architecture::Architecture,
        Kind::Block => &block::Block,
        Kind::C4 => &c4::C4,
        _ => return None,
    })
}
