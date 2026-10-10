//! Mermaid `pie` and `xychart-beta` source as a chart, parsed by
//! `mermaid-rs-renderer`.
//!
//! An xychart's axis titles and y range are dropped. One whose series mix
//! bars and lines answers `None`.

use std::panic::{self, AssertUnwindSafe};

use mermaid_rs_renderer as mmdr;

use crate::{
    data::Data,
    model::{Chart, Kind},
};

/// The chart `source` describes, or `None` for source that does not parse or
/// is another kind of diagram.
pub fn import(source: &str) -> Option<Chart> {
    // The parser sees every keystroke of a fence being typed. A panic in it is
    // a chart that does not draw, not an app that goes down.
    let parsed = panic::catch_unwind(AssertUnwindSafe(|| mmdr::parse_mermaid_strict(source).ok()))
        .ok()??;
    let graph = parsed.graph;
    match graph.kind {
        mmdr::DiagramKind::Pie => pie(&graph),
        mmdr::DiagramKind::XYChart => xy(&graph.xychart),
        _ => None,
    }
}

fn pie(graph: &mmdr::Graph) -> Option<Chart> {
    if graph.pie_slices.is_empty() {
        return None;
    }
    let data = Data::new()
        .text("label", graph.pie_slices.iter().map(|s| s.label.clone()))
        .number(
            "value",
            graph
                .pie_slices
                .iter()
                .map(|s| f64::from(s.value))
                .collect::<Vec<_>>(),
        );
    let chart = Chart::arc(data)
        .theta("value", Kind::Quantitative)
        .color("label", Kind::Nominal);
    Some(match &graph.pie_title {
        Some(title) => chart.title(title.clone()),
        None => chart,
    })
}

fn xy(xy: &mmdr::ir::XYChartData) -> Option<Chart> {
    let kind = xy.series.first()?.kind;
    if xy.series.iter().any(|series| series.kind != kind) {
        return None;
    }
    let length = xy.series.iter().map(|s| s.values.len()).max()?;
    let category = |index: usize| match xy.x_axis_categories.get(index) {
        Some(name) => name.clone(),
        None => (index + 1).to_string(),
    };
    let (mut categories, mut names, mut values) = (Vec::new(), Vec::new(), Vec::new());
    for (index, series) in xy.series.iter().enumerate() {
        let name = series
            .label
            .clone()
            .unwrap_or_else(|| format!("Series {}", index + 1));
        for (at, &value) in series.values.iter().enumerate().take(length) {
            categories.push(category(at));
            names.push(name.clone());
            values.push(f64::from(value));
        }
    }
    let data = Data::new()
        .text("x", categories)
        .text("series", names)
        .number("y", values);
    let chart = match kind {
        mmdr::ir::XYSeriesKind::Bar => Chart::bar(data).x("x", Kind::Nominal),
        mmdr::ir::XYSeriesKind::Line => Chart::line(data).x("x", Kind::Ordinal),
    }
    .y("y", Kind::Quantitative);
    let chart = match xy.series.len() > 1 {
        true => chart.color("series", Kind::Nominal),
        false => chart,
    };
    Some(match &xy.title {
        Some(title) => chart.title(title.clone()),
        None => chart,
    })
}
