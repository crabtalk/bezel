//! Every diagram type, and the one a source names.

use serde::Serialize;

use super::{Config, Ctx, Error, Host, Kind, ParseLimits, Rendered};

pub mod agentflow;
pub mod architecture;
pub mod block;
pub mod c4;
pub mod class;
pub mod common;
pub mod cynefin;
pub mod er;
pub mod eventmodeling;
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
pub mod swimlane;
pub mod timeline;
pub mod treemap;
pub mod treeview;
pub mod usecase;
pub mod venn;
pub mod wardley;
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
    Cynefin(cynefin::CynefinModel),
    Wardley(Box<wardley::WardleyModel>),
    EventModeling(Box<eventmodeling::EventModelingModel>),
    Architecture(Box<architecture::ArchitectureModel>),
    Block(Box<block::BlockModel>),
    C4(c4::C4Model),
    Agentflow(Box<agentflow::AgentflowDb>),
    Usecase(Box<usecase::UsecaseModel>),
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
        Kind::Cynefin => &cynefin::Cynefin,
        Kind::Wardley => &wardley::Wardley,
        Kind::Eventmodeling => &eventmodeling::EventModeling,
        Kind::Architecture => &architecture::Architecture,
        Kind::Block => &block::Block,
        Kind::C4 => &c4::C4,
        Kind::Swimlane => &swimlane::Swimlane,
        Kind::Agentflow => &agentflow::Agentflow,
        Kind::Usecase => &usecase::Usecase,
    })
}

/// `model`, of type `kind`, drawn.
pub fn render(
    kind: Kind,
    model: &Model,
    config: &Config,
    ctx: &Ctx<'_, impl Host>,
) -> Result<Rendered, Error> {
    match kind {
        Kind::GitGraph => git::render(model, config, ctx),
        Kind::Gantt => gantt::render(model, config, ctx),
        Kind::Info => info::render(model, config, ctx),
        Kind::Flowchart => flowchart::render(model, config, ctx),
        Kind::XyChart => xychart::render(model, config, ctx),
        Kind::QuadrantChart => quadrant::render(model, config, ctx),
        Kind::Sankey => sankey::render(model, config, ctx),
        Kind::Treemap => treemap::render(model, config, ctx),
        Kind::Radar => radar::render(model, config, ctx),
        Kind::Pie => pie::render(model, config, ctx),
        Kind::Mindmap => mindmap::render(model, config, ctx),
        Kind::Packet => packet::render(model, config, ctx),
        Kind::TreeView => treeview::render(model, config, ctx),
        Kind::Ishikawa => ishikawa::render(model, config, ctx),
        Kind::Railroad => railroad::render(model, config, ctx),
        Kind::Venn => venn::render(model, config, ctx),
        Kind::Journey => journey::render(model, config, ctx),
        Kind::Kanban => kanban::render(model, config, ctx),
        Kind::Timeline => timeline::render(model, config, ctx),
        Kind::Class => class::render(model, config, ctx),
        Kind::State => state::render(model, config, ctx),
        Kind::Er => er::render(model, config, ctx),
        Kind::Requirement => requirement::render(model, config, ctx),
        Kind::Sequence => sequence::render(model, config, ctx),
        Kind::Cynefin => cynefin::render(model, config, ctx),
        Kind::Wardley => wardley::render(model, config, ctx),
        Kind::Eventmodeling => eventmodeling::render(model, config, ctx),
        Kind::Architecture => architecture::render(model, config, ctx),
        Kind::Block => block::render(model, config, ctx),
        Kind::C4 => c4::render(model, config, ctx),
        Kind::Swimlane => swimlane::render(model, config, ctx),
        Kind::Agentflow => agentflow::render(model, config, ctx),
        Kind::Usecase => usecase::render(model, config, ctx),
    }
}
