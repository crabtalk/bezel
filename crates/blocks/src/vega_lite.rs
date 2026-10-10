//! ` ```vega-lite ` — a Vega-Lite spec, drawn by `bezel-chart` for the subset
//! [`chart::vega_lite`] reads.
//!
//! Parsing costs milliseconds and paint runs every frame, so each source is
//! parsed once and its chart kept. The kept chart's data generation is what
//! keeps the element's layout from one frame to the next.

use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
};

use chart::Chart;
use gpui::{AnyElement, App, Global, SharedString, Window, div, prelude::*, px};
use theme::Theme;

pub const LANGUAGE: &str = "vega-lite";

/// Charts kept parsed. Past this the cache starts over.
const CACHED: usize = 64;
/// The block's height when the fence states none.
const HEIGHT: f32 = 260.0;
const PADDING: f32 = 12.0;

/// Parsed charts by source. `None` for source that did not parse, so a fence
/// being typed is not parsed again every frame.
#[derive(Default)]
struct Cache(HashMap<u64, Option<Chart>>);

impl Global for Cache {}

pub fn render(code: &str, height: Option<u32>, _: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let mut hasher = DefaultHasher::new();
    code.hash(&mut hasher);
    let key = hasher.finish();
    let cache = &mut cx.default_global::<Cache>().0;
    let chart = match cache.get(&key) {
        Some(chart) => chart.clone(),
        None => {
            if cache.len() >= CACHED {
                cache.clear();
            }
            let chart = chart::vega_lite::import(code);
            cache.insert(key, chart.clone());
            chart
        }
    }?;

    let theme = Theme::of(cx);
    let id = SharedString::from(format!("vega-lite-{key:x}"));
    Some(
        div()
            .debug_selector(|| LANGUAGE.into())
            .h(px(height.map_or(HEIGHT, |height| height as f32)))
            .p(px(PADDING))
            .rounded(px(Theme::BASE_RADIUS))
            .bg(theme.ink(0.02))
            .child(chart::view(id, chart).size_full())
            .into_any_element(),
    )
}
