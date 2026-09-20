//! The client-decoration geometry — which band a press lands in.
//!
//! The frame itself paints only under `Decorations::Client`, which macOS and
//! Windows never report, so what a test can reach here is the classification
//! the bands and the press handler share.

use gpui::{Pixels, ResizeEdge, point, px, size};
use ui::window::resize_edge;

/// A 200 x 100 window with a 10px band.
fn edge_at(x: f32, y: f32) -> Option<ResizeEdge> {
    resize_edge(point(px(x), px(y)), px(10.0), size(px(200.0), px(100.0)))
}

#[test]
fn a_press_on_the_content_resizes_nothing() {
    assert_eq!(edge_at(100.0, 50.0), None);
    // The band is exclusive at its inner edge: 10 is already content.
    assert_eq!(edge_at(10.0, 50.0), None);
    assert_eq!(edge_at(190.0, 50.0), None);
}

#[test]
fn each_side_resizes_its_own_edge() {
    assert_eq!(edge_at(100.0, 2.0), Some(ResizeEdge::Top));
    assert_eq!(edge_at(100.0, 98.0), Some(ResizeEdge::Bottom));
    assert_eq!(edge_at(2.0, 50.0), Some(ResizeEdge::Left));
    assert_eq!(edge_at(198.0, 50.0), Some(ResizeEdge::Right));
}

#[test]
fn a_corner_beats_the_two_edges_it_lies_in() {
    assert_eq!(edge_at(2.0, 2.0), Some(ResizeEdge::TopLeft));
    assert_eq!(edge_at(198.0, 2.0), Some(ResizeEdge::TopRight));
    assert_eq!(edge_at(2.0, 98.0), Some(ResizeEdge::BottomLeft));
    assert_eq!(edge_at(198.0, 98.0), Some(ResizeEdge::BottomRight));
}

/// A window narrower than two bands has overlapping sides, and every press in
/// it is still exactly one corner — never `None`, and never an edge.
#[test]
fn a_window_thinner_than_its_bands_is_all_corner() {
    let corners = [
        ResizeEdge::TopLeft,
        ResizeEdge::TopRight,
        ResizeEdge::BottomLeft,
        ResizeEdge::BottomRight,
    ];
    for x in 0..12 {
        for y in 0..12 {
            let edge = resize_edge(
                point(px(x as f32), px(y as f32)),
                px(10.0),
                size(px(12.0), px(12.0)),
            );
            assert!(
                edge.is_some_and(|edge| corners.contains(&edge)),
                "({x}, {y}) resized {edge:?}"
            );
        }
    }
}

/// The inset gpui is handed and the inset the press is classified against are
/// the same number, so a window that opens at the theme's own band answers for
/// a press one pixel inside it.
#[test]
fn the_band_is_the_theme_constant() {
    let inset: Pixels = px(theme::Theme::CLIENT_INSET);
    let window = size(px(400.0), px(300.0));
    assert_eq!(
        resize_edge(point(inset - px(1.0), px(150.0)), inset, window),
        Some(ResizeEdge::Left)
    );
    assert_eq!(resize_edge(point(inset, px(150.0)), inset, window), None);
}
