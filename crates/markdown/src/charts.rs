//! A fence drawn as a chart: ` ```mermaid ` pie and xychart source under the
//! `mermaid` feature, and ` ```vega-lite ` under `chart`.
//!
//! Parsing costs milliseconds and paint runs every frame, so each source is
//! parsed once and its chart kept. The kept chart's data generation is what
//! keeps the element's layout from one frame to the next.
//!
//! A press on the chart does not reach the editor; the band above it does,
//! which is how the source is reached for editing.

use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
};

use chart::Chart;
use gpui::{AnyElement, App, Global, MouseButton, SharedString, Window, div, prelude::*, px};
use theme::Theme;

use crate::render::{copy_button, fence_band, fence_panel};

#[cfg(feature = "chart")]
pub const VEGA_LITE: &str = "vega-lite";

/// The debug selector on a drawn chart.
pub const SELECTOR: &str = "chart";

/// Charts kept parsed. Past this the cache starts over.
const CACHED: usize = 64;

/// The chart's height when the fence states none.
const HEIGHT: f32 = 260.0;

/// Parsed charts by language and source. `None` for source that did not
/// parse, so a fence being typed is not parsed again every frame.
#[derive(Default)]
struct Cache(HashMap<u64, Option<Chart>>);

impl Global for Cache {}

fn parsed(key: u64, code: &str, parse: fn(&str) -> Option<Chart>, cx: &mut App) -> Option<Chart> {
    let cache = &mut cx.default_global::<Cache>().0;
    if let Some(chart) = cache.get(&key) {
        return chart.clone();
    }
    if cache.len() >= CACHED {
        cache.clear();
    }
    let chart = parse(code);
    cache.insert(key, chart.clone());
    chart
}

/// The chart `parse` reads from `code`, in a fence labelled `language`, or
/// `None` to leave the fence to its source.
pub(crate) fn render(
    language: &'static str,
    code: &str,
    parse: fn(&str) -> Option<Chart>,
    height: Option<u32>,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyElement> {
    let mut hasher = DefaultHasher::new();
    (language, code).hash(&mut hasher);
    let key = hasher.finish();
    let chart = parsed(key, code, parse, cx)?;
    let theme = Theme::of(cx).clone();
    let id = SharedString::from(format!("md-chart-{key:x}"));

    let body = div()
        .id(id.clone())
        .debug_selector(|| SELECTOR.into())
        .w_full()
        .map(|el| match height {
            Some(_) => el.flex_1().min_h_0(),
            None => el.h(px(HEIGHT)),
        })
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(chart::view(id, chart).size_full());

    Some(
        fence_panel(&theme)
            .when_some(height, |el, height| {
                el.h(px(height as f32)).flex().flex_col()
            })
            .child(
                fence_band(&theme)
                    .flex_none()
                    .text_color(theme.text_muted)
                    .child(language),
            )
            .child(body)
            .child(copy_button(code, key as usize, &theme, window, cx))
            .into_any_element(),
    )
}
