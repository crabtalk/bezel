//! ` ```mermaid ` — the diagram, drawn as a read-only canvas.
//!
//! Parsing and layout cost milliseconds and paint runs every frame, so each
//! source is laid out once per text size and kept.

use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    rc::Rc,
};

use canvas_core::Canvas;
use gpui::{AnyElement, App, Global, SharedString, div, prelude::*};
use theme::TextStyle;

pub const LANGUAGE: &str = "mermaid";

/// Diagrams kept laid out. Past this the cache starts over.
const CACHED: usize = 64;

/// Laid-out diagrams by source and text size. `None` for source that did not
/// lay out, so a fence being typed is not parsed again every frame.
#[derive(Default)]
struct Cache(HashMap<u64, Option<Rc<Canvas>>>);

impl Global for Cache {}

/// The diagram `code` describes, or `None` to leave the fence to its source.
pub fn render(code: &str, cx: &mut App) -> Option<AnyElement> {
    let size = TextStyle::Callout.painted();
    let key = {
        let mut hasher = DefaultHasher::new();
        (code, size.to_bits()).hash(&mut hasher);
        hasher.finish()
    };
    let cache = &mut cx.default_global::<Cache>().0;
    let canvas = match cache.get(&key) {
        Some(canvas) => canvas.clone(),
        None => {
            if cache.len() >= CACHED {
                cache.clear();
            }
            let canvas = canvas_core::mermaid::import(code, size).map(Rc::new);
            cache.insert(key, canvas.clone());
            canvas
        }
    }?;
    Some(
        div()
            .id(SharedString::from(format!("mermaid-{key:x}")))
            .debug_selector(|| LANGUAGE.into())
            .w_full()
            .overflow_x_scroll()
            .child(canvas_core::diagram(&canvas, cx))
            .into_any_element(),
    )
}
