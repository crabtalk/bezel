//! ` ```vega-lite ` — a Vega-Lite spec, drawn by `bezel-chart` for the subset
//! [`chart::vega_lite`] reads.
//!
//! Parsing costs milliseconds and paint runs every frame, so each source is
//! parsed once and its chart kept. The kept chart's data generation is what
//! keeps the element's layout from one frame to the next.
//!
//! A spec with a `data.url` paints once the [`Loader`] installed with
//! [`set_loader`] has text for every url it names. Until then, and with no
//! loader installed, the fence shows its source.

use chart::{Chart, vega_lite::Spec};
use gpui::{AnyElement, App, Global, SharedString, Window, div, prelude::*, px};
use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    rc::Rc,
};
use theme::Theme;

pub const LANGUAGE: &str = "vega-lite";

/// Charts kept parsed. Past this the cache starts over.
const CACHED: usize = 64;
/// The block's height when the fence states none.
const HEIGHT: f32 = 260.0;
const PADDING: f32 = 12.0;

/// Answers a spec's `data.url`, as written, with the text at it. Asked at
/// paint. `None` for a url the app has nothing for yet: the fence shows its
/// source, and asks again when the app next repaints it.
pub type Loader = fn(&str, &App) -> Option<SharedString>;

struct Installed(Loader);

impl Global for Installed {}

pub fn set_loader(cx: &mut App, loader: Loader) {
    cx.set_global(Installed(loader));
}

/// Charts by source. `Drawn(None)` for source that did not parse, so a fence
/// being typed is not parsed again every frame.
#[derive(Default)]
struct Cache(HashMap<u64, Entry>);

impl Global for Cache {}

#[derive(Clone)]
enum Entry {
    Drawn(Option<Chart>),
    /// A spec whose urls the loader has not answered yet.
    Waiting(Rc<Spec>),
}

pub fn render(code: &str, height: Option<u32>, _: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let mut hasher = DefaultHasher::new();
    code.hash(&mut hasher);
    let key = hasher.finish();
    let cache = &mut cx.default_global::<Cache>().0;
    let entry = match cache.get(&key) {
        Some(entry) => entry.clone(),
        None => {
            if cache.len() >= CACHED {
                cache.clear();
            }
            let entry = match code.parse::<Spec>() {
                Err(_) => Entry::Drawn(None),
                Ok(spec) if spec.urls().is_empty() => Entry::Drawn(spec.chart(&|_| None)),
                Ok(spec) => Entry::Waiting(Rc::new(spec)),
            };
            cache.insert(key, entry.clone());
            entry
        }
    };
    let chart = match entry {
        Entry::Drawn(chart) => chart?,
        Entry::Waiting(spec) => {
            let loader = cx.try_global::<Installed>()?.0;
            let texts: HashMap<&str, SharedString> = spec
                .urls()
                .into_iter()
                .map(|url| Some((url, loader(url, cx)?)))
                .collect::<Option<_>>()?;
            let chart = spec.chart(&|url| texts.get(url).map(SharedString::as_ref));
            cx.default_global::<Cache>()
                .0
                .insert(key, Entry::Drawn(chart.clone()));
            chart?
        }
    };

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
