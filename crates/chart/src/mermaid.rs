//! Mermaid `pie` and `xychart-beta` source as a chart, parsed by
//! `mermaid-rs-renderer`.
//!
//! An xychart keeps its categories in the order written, and its axis titles
//! and y range. Its bars are one layer, stacked, and its lines another over
//! them.

use std::panic::{self, AssertUnwindSafe};

use mermaid_rs_renderer as mmdr;

use crate::{
    data::Data,
    model::{Channel, Chart},
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
        .theta(Channel::quantitative("value"))
        .color(Channel::nominal("label"));
    Some(match &graph.pie_title {
        Some(title) => chart.title(title.clone()),
        None => chart,
    })
}

fn xy(xy: &mmdr::ir::XYChartData) -> Option<Chart> {
    let category = |index: usize| match xy.x_axis_categories.get(index) {
        Some(name) => name.clone(),
        None => (index + 1).to_string(),
    };
    let name = |index: usize| match &xy.series[index].label {
        Some(label) => label.clone(),
        None => format!("Series {}", index + 1),
    };
    let mut x = Channel::nominal("x");
    x.title = xy.x_axis_label.clone().map(Into::into);
    let mut y = Channel::quantitative("y");
    y.title = xy.y_axis_label.clone().map(Into::into);
    if let (Some(low), Some(high)) = (xy.y_axis_min, xy.y_axis_max) {
        y = y.domain(f64::from(low), f64::from(high));
    }
    let of_kind = |kind| -> Vec<usize> {
        (0..xy.series.len())
            .filter(|&index| xy.series[index].kind == kind)
            .collect()
    };
    let (bars, lines) = (
        of_kind(mmdr::ir::XYSeriesKind::Bar),
        of_kind(mmdr::ir::XYSeriesKind::Line),
    );
    // A colour field where a layer holds several series or a series is
    // named; otherwise each layer's own colour tells them apart.
    let colored =
        bars.len() > 1 || lines.len() > 1 || xy.series.iter().any(|series| series.label.is_some());

    let layer = |series: &[usize], chart: fn(Data) -> Chart| {
        let (mut categories, mut names, mut values) = (Vec::new(), Vec::new(), Vec::new());
        for &index in series {
            for (at, &value) in xy.series[index].values.iter().enumerate() {
                categories.push(category(at));
                names.push(name(index));
                values.push(f64::from(value));
            }
        }
        let data = Data::new()
            .text("x", categories)
            .text("series", names)
            .number("y", values);
        let layer = chart(data).x(x.clone()).y(y.clone());
        match colored {
            true => layer.color(Channel::nominal("series")),
            false => layer,
        }
    };
    let chart = match (bars.is_empty(), lines.is_empty()) {
        (true, true) => return None,
        (false, true) => layer(&bars, Chart::bar),
        (true, false) => layer(&lines, Chart::line),
        (false, false) => layer(&bars, Chart::bar).layer(layer(&lines, Chart::line)),
    };
    Some(match &xy.title {
        Some(title) => chart.title(title.clone()),
        None => chart,
    })
}
