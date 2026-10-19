//! `bezel-mermaid` for the web, under Pele 1.6.0's API: `render`, `parse`,
//! `supports`, `detectType` and `forgetTextWidths`. The options take Pele's
//! names. A failure throws an `Error` carrying Pele's `code`, `type`, `line`,
//! `column` and `snippet`.
//!
//! Text is measured on a canvas, in `fontFamily` (`sans-serif` unless set)
//! and `monospace`, or by `measurer` when the page passes one. Where there is
//! no canvas, or with `measure: "metrics"`, it is measured by Arial's
//! advances, as Pele does outside a browser.

mod canvas;

use std::{cell::RefCell, collections::HashMap};

use js_sys::{Array, Function, JSON, Object, Reflect};
use mermaid::{Error, Host, Metrics, Options};
use wasm_bindgen::prelude::*;

use crate::canvas::{Canvas, Custom};

/// Draws `source`, as Pele's `render`: `{ svg, width, height, type, links }`.
#[wasm_bindgen]
pub fn render(source: &str, options: JsValue) -> Result<JsValue, JsValue> {
    let fields = Fields::of(&options);
    let options = read_options(&fields, true)?;
    let page = Page::of(&fields, options.font_family.as_deref());
    let drawing = mermaid::render(source, &options, &page).map_err(thrown)?;
    let out = Object::new();
    set(&out, "svg", &drawing.svg.into());
    set(&out, "width", &drawing.width.into());
    set(&out, "height", &drawing.height.into());
    set(&out, "type", &drawing.kind.name().into());
    let links = Array::new();
    for link in drawing.links {
        let item = Object::new();
        set(&item, "id", &link.id.into());
        set(&item, "href", &link.href.into());
        set(&item, "internal", &link.internal.into());
        links.push(&item);
    }
    set(&out, "links", &links);
    Ok(out.into())
}

/// The model `source` parses to, as Pele's `parse`. Reads `limit`,
/// `maxEdges` and `config`.
#[wasm_bindgen]
pub fn parse(source: &str, options: JsValue) -> Result<JsValue, JsValue> {
    let options = read_options(&Fields::of(&options), false)?;
    let parsed = mermaid::parse(source, &options).map_err(thrown)?;
    let text = serde_json::to_string(&parsed.model).map_err(|error| error.to_string())?;
    JSON::parse(&text)
}

/// Whether `source` names a type this build draws.
#[wasm_bindgen]
pub fn supports(source: &str) -> bool {
    mermaid::supports(source)
}

/// The type `source` names, or `null`.
#[wasm_bindgen(js_name = detectType)]
pub fn detect_type(source: &str) -> Option<String> {
    mermaid::detect_type(source).map(|kind| kind.name().to_string())
}

/// Forgets the widths the canvas measured, as Pele's `forgetTextWidths`:
/// for after a font the page uses has loaded.
#[wasm_bindgen(js_name = forgetTextWidths)]
pub fn forget_text_widths() {
    canvas::forget();
}

fn set(target: &Object, key: &str, value: &JsValue) {
    let _ = Reflect::set(target, &key.into(), value);
}

/// An options object's own enumerable properties, as `{ ...options }`
/// reads them, without the undefined and null ones.
struct Fields(Vec<(String, JsValue)>);

impl Fields {
    fn of(options: &JsValue) -> Self {
        if !options.is_object() {
            return Self(Vec::new());
        }
        let fields = Object::entries(options.unchecked_ref())
            .iter()
            .filter_map(|entry| {
                let entry: Array = entry.unchecked_into();
                let value = entry.get(1);
                if value.is_undefined() || value.is_null() {
                    return None;
                }
                Some((entry.get(0).as_string()?, value))
            })
            .collect();
        Self(fields)
    }

    fn get(&self, key: &str) -> Option<JsValue> {
        self.0
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.clone())
    }

    fn string(&self, key: &str) -> Option<String> {
        self.get(key).and_then(|value| value.as_string())
    }

    fn number(&self, key: &str) -> Option<f64> {
        let value = self.get(key)?;
        if let Some(date) = value.dyn_ref::<js_sys::Date>() {
            return Some(date.get_time());
        }
        value.as_f64()
    }

    fn count(&self, key: &str) -> Option<usize> {
        self.number(key)
            .filter(|n| n.is_finite() && *n >= 0.0)
            .map(|n| n as usize)
    }

    fn flag(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(|value| value.as_bool())
    }

    fn strings(&self, key: &str) -> Option<Vec<String>> {
        let value = self.get(key)?;
        let array = value.dyn_ref::<Array>()?;
        Some(array.iter().filter_map(|item| item.as_string()).collect())
    }
}

/// Pele's `RenderOptions`, or for `parse` only what parsing reads.
fn read_options(options: &Fields, drawing: bool) -> Result<Options, JsValue> {
    let config = match options.get("config") {
        Some(value) => {
            let text = JSON::stringify(&value)?.as_string().unwrap_or_default();
            match mermaid::js::parse_json(&text) {
                Some(mermaid::Value::Object(map)) => Some(map),
                _ => None,
            }
        }
        None => None,
    };
    let mut out = Options::default();
    if let Some(config) = config {
        out.config = config;
    }
    if let Some(limit) = options.count("limit") {
        out.limit = limit;
    }
    if let Some(max_edges) = options.count("maxEdges") {
        out.max_edges = max_edges;
    }
    if !drawing {
        return Ok(out);
    }
    out.font_family = options.string("fontFamily");
    out.font_family_mono = options.string("fontFamilyMono");
    if let Some(size) = options.number("fontSize") {
        out.font_size = size;
    }
    if let Some(prefix) = options.string("idPrefix") {
        out.id_prefix = prefix;
    }
    out.responsive = options.flag("responsive");
    out.max_width = options.number("maxWidth");
    if let Some(auto) = options.flag("autoDirection") {
        out.auto_direction = auto;
    }
    if let Some(breakpoint) = options.number("directionBreakpoint") {
        out.direction_breakpoint = breakpoint;
    }
    out.padding = options.number("padding");
    if let Some(limit) = options.count("outputLimit") {
        out.output_limit = limit;
    }
    if let Some(schemes) = options.strings("linkSchemes") {
        out.link_schemes = schemes;
    }
    if let Some(schemes) = options.strings("imageSchemes") {
        out.image_schemes = schemes;
    }
    if let Some(links) = options.flag("links") {
        out.links = links;
    }
    if let Some(images) = options.flag("images") {
        out.images = images;
    }
    if let Some(rel) = options.string("linkRel") {
        out.link_rel = rel;
    }
    out.now = options.number("now");
    Ok(out)
}

/// What labels are measured with: `measurer` when the page passes one, else
/// the canvas unless `measure` is `"metrics"` or there is none.
enum Measurer {
    Custom(Custom),
    Canvas(Canvas),
    Metrics,
}

/// The page a drawing is made for: its measurer, and its `icons` resolver,
/// each answer kept for the render.
struct Page {
    measurer: Measurer,
    icons: Option<Function>,
    found: RefCell<HashMap<String, Option<String>>>,
}

impl Page {
    fn of(options: &Fields, family: Option<&str>) -> Self {
        let measurer = if let Some(custom) = options.get("measurer").and_then(Custom::of) {
            Measurer::Custom(custom)
        } else if options.string("measure").as_deref() == Some("metrics") {
            Measurer::Metrics
        } else {
            Canvas::new(family.unwrap_or("sans-serif"), "monospace")
                .map_or(Measurer::Metrics, Measurer::Canvas)
        };
        Self {
            measurer,
            icons: options
                .get("icons")
                .and_then(|value| value.dyn_into::<Function>().ok()),
            found: RefCell::default(),
        }
    }
}

impl Host for Page {
    fn width(&self, text: &str, size: f64, style: u8) -> f64 {
        match &self.measurer {
            Measurer::Custom(custom) => custom.width(text, size, style),
            Measurer::Canvas(canvas) => canvas.width(text, size, style),
            Measurer::Metrics => Metrics.width(text, size, style),
        }
    }

    fn icon(&self, name: &str) -> Option<String> {
        let resolve = self.icons.as_ref()?;
        if let Some(found) = self.found.borrow().get(name) {
            return found.clone();
        }
        let markup = resolve
            .call1(&JsValue::NULL, &name.into())
            .ok()
            .and_then(|markup| markup.as_string());
        self.found
            .borrow_mut()
            .insert(name.to_string(), markup.clone());
        markup
    }
}

/// `error` as the `Error` Pele throws.
fn thrown(error: Error) -> JsValue {
    let out = js_sys::Error::new(&error.message);
    out.set_name("PeleError");
    let code = match error.code {
        mermaid::Code::UnsupportedDiagram => "unsupported-diagram",
        mermaid::Code::Syntax => "syntax",
        mermaid::Code::Semantic => "semantic",
        mermaid::Code::Limit => "limit",
    };
    set(&out, "code", &code.into());
    set(
        &out,
        "type",
        &error.kind.map_or(JsValue::NULL, |kind| kind.name().into()),
    );
    set(&out, "line", &(error.line as f64).into());
    set(&out, "column", &(error.column as f64).into());
    set(&out, "snippet", &error.snippet.into());
    out.into()
}
