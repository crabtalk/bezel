//! Labels measured as Pele's `canvasMeasurer` measures them: a 2D canvas's
//! `measureText` in a CSS font, a width kept per font and text. A width
//! measured while the font was not yet loaded is not kept, and every kept
//! width is forgotten when the document's font faces change.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};

use js_sys::{Function, Reflect};
use mermaid::{Host, js, text::Style};
use wasm_bindgen::prelude::*;

/// Widths kept per font. Past this the font's widths start over.
const MAX_CACHED: usize = 20_000;

thread_local! {
    /// Widths by CSS font, then text.
    static WIDTHS: RefCell<HashMap<String, HashMap<String, f64>>> = RefCell::new(HashMap::new());
    /// How many font faces the document had when the widths were measured.
    static FACES: Cell<f64> = const { Cell::new(f64::NAN) };
}

#[wasm_bindgen(inline_js = r#"
let ctx;
function context() {
  if (ctx !== undefined) return ctx;
  ctx = null;
  try {
    if (globalThis.OffscreenCanvas) ctx = new OffscreenCanvas(1, 1).getContext('2d');
    else if (globalThis.document) ctx = document.createElement('canvas').getContext('2d');
  } catch {
    ctx = null;
  }
  return ctx;
}
function fonts() {
  return globalThis.document?.fonts ?? globalThis.fonts;
}
export function has_canvas() {
  return context() !== null;
}
export function font_faces() {
  const size = fonts()?.size;
  return size === undefined ? -1 : size;
}
let current = '';
export function measure_text(text, font) {
  const c = context();
  if (font !== current) c.font = current = font;
  const width = c.measureText(text).width;
  let loaded = true;
  try {
    loaded = fonts()?.check(font, text) ?? true;
  } catch {
    loaded = true;
  }
  return loaded ? width : -width - 1;
}
"#)]
extern "C" {
    fn has_canvas() -> bool;
    fn font_faces() -> f64;
    /// The width, or `-width - 1` for one measured in a fallback.
    fn measure_text(text: &str, font: &str) -> f64;
}

/// Forgets every kept width.
pub fn forget() {
    WIDTHS.with(|widths| widths.borrow_mut().clear());
}

pub struct Canvas {
    family: String,
    mono: String,
}

impl Canvas {
    /// `None` where the page has no canvas.
    pub fn new(family: &str, mono: &str) -> Option<Self> {
        has_canvas().then(|| Self {
            family: family.to_string(),
            mono: mono.to_string(),
        })
    }
}

impl Host for Canvas {
    fn width(&self, text: &str, size: f64, style: u8) -> f64 {
        let faces = font_faces();
        if FACES.get() != faces {
            FACES.set(faces);
            forget();
        }
        let font = format!(
            "{}{}{}px {}",
            if style & Style::ITALIC != 0 {
                "italic "
            } else {
                ""
            },
            if style & Style::BOLD != 0 {
                "bold "
            } else {
                ""
            },
            js::num_str(size),
            if style & Style::MONO != 0 {
                &self.mono
            } else {
                &self.family
            },
        );
        let kept = WIDTHS.with(|widths| widths.borrow().get(&font)?.get(text).copied());
        if let Some(width) = kept {
            return width;
        }
        let measured = measure_text(text, &font);
        if measured < 0.0 {
            return -measured - 1.0;
        }
        WIDTHS.with(|widths| {
            let mut widths = widths.borrow_mut();
            let widths = widths.entry(font).or_default();
            if widths.len() >= MAX_CACHED {
                widths.clear();
            }
            widths.insert(text.to_string(), measured);
        });
        measured
    }
}

/// A page's own `measurer`: an object with `width(text, size, style)`.
pub struct Custom {
    target: JsValue,
    width: Function,
}

impl Custom {
    pub fn of(target: JsValue) -> Option<Self> {
        let width = Reflect::get(&target, &"width".into())
            .ok()?
            .dyn_into::<Function>()
            .ok()?;
        Some(Self { target, width })
    }
}

impl Host for Custom {
    fn width(&self, text: &str, size: f64, style: u8) -> f64 {
        self.width
            .call3(&self.target, &text.into(), &size.into(), &style.into())
            .ok()
            .and_then(|width| width.as_f64())
            .unwrap_or(0.0)
    }
}
