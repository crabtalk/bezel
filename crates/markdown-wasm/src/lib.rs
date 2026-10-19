//! `bezel-markdown`'s Mermaid engine for the web, under Pele 1.6.0's API:
//! `render`, `parse`, `supports`, `detectType` and `forgetTextWidths`. The
//! options take Pele's names. A failure throws an `Error` carrying Pele's
//! `code`, `type`, `line`, `column` and `snippet`.
//!
//! Text is measured on a canvas, in `fontFamily` (`sans-serif` unless set)
//! and `monospace`, or by `measurer` when the page passes one. Where there is
//! no canvas, or with `measure: "metrics"`, it is measured by Arial's
//! advances, as Pele does outside a browser.

mod canvas;

use std::rc::Rc;

use js_sys::{Array, Function, JSON, Object, Reflect};
use markdown::mermaid::{self, Error, IconResolver, Measure, Metrics, Options, config};
use wasm_bindgen::prelude::*;

use crate::canvas::{Canvas, Custom};

/// Draws `source`, as Pele's `render`: `{ svg, width, height, type, links }`.
#[wasm_bindgen]
pub fn render(source: &str, options: JsValue) -> Result<JsValue, JsValue> {
    let options = read_options(&options, true)?;
    let drawing = mermaid::render(source, &options).map_err(thrown)?;
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
    let options = read_options(&options, false)?;
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

fn get(options: &JsValue, key: &str) -> Option<JsValue> {
    if !options.is_object() {
        return None;
    }
    Reflect::get(options, &key.into())
        .ok()
        .filter(|value| !value.is_undefined() && !value.is_null())
}

fn string(options: &JsValue, key: &str) -> Option<String> {
    get(options, key).and_then(|value| value.as_string())
}

fn number(options: &JsValue, key: &str) -> Option<f64> {
    let value = get(options, key)?;
    if let Some(date) = value.dyn_ref::<js_sys::Date>() {
        return Some(date.get_time());
    }
    value.as_f64()
}

fn count(options: &JsValue, key: &str) -> Option<usize> {
    number(options, key)
        .filter(|n| n.is_finite() && *n >= 0.0)
        .map(|n| n as usize)
}

fn flag(options: &JsValue, key: &str) -> Option<bool> {
    get(options, key).and_then(|value| value.as_bool())
}

fn strings(options: &JsValue, key: &str) -> Option<Vec<String>> {
    let value = get(options, key)?;
    let array = value.dyn_ref::<Array>()?;
    Some(array.iter().filter_map(|item| item.as_string()).collect())
}

/// Pele's `RenderOptions`, or for `parse` only what parsing reads.
fn read_options(options: &JsValue, drawing: bool) -> Result<Options, JsValue> {
    let config = match get(options, "config") {
        Some(value) => {
            let text = JSON::stringify(&value)?.as_string().unwrap_or_default();
            match config::parse_json(&text) {
                Some(mermaid::Value::Map(map)) => Some(map),
                _ => None,
            }
        }
        None => None,
    };
    let mut out = Options {
        limit: count(options, "limit"),
        max_edges: count(options, "maxEdges"),
        config,
        ..Options::default()
    };
    if !drawing {
        return Ok(out);
    }
    out.font_family = string(options, "fontFamily");
    out.font_family_mono = string(options, "fontFamilyMono");
    out.font_size = number(options, "fontSize");
    out.id_prefix = string(options, "idPrefix");
    out.responsive = flag(options, "responsive");
    out.max_width = number(options, "maxWidth");
    out.auto_direction = flag(options, "autoDirection");
    out.direction_breakpoint = number(options, "directionBreakpoint");
    out.padding = number(options, "padding");
    out.output_limit = count(options, "outputLimit");
    out.link_schemes = strings(options, "linkSchemes");
    out.image_schemes = strings(options, "imageSchemes");
    out.links = flag(options, "links");
    out.images = flag(options, "images");
    out.link_rel = string(options, "linkRel");
    out.now = number(options, "now");
    out.icons = get(options, "icons")
        .and_then(|value| value.dyn_into::<Function>().ok())
        .map(|resolve| -> IconResolver {
            Rc::new(move |name: &str| {
                resolve
                    .call1(&JsValue::NULL, &name.into())
                    .ok()
                    .and_then(|markup| markup.as_string())
            })
        });
    out.measure = Some(measurer(options, out.font_family.as_deref()));
    Ok(out)
}

/// `measurer` when the page passes one, else the canvas unless `measure` is
/// `"metrics"` or there is none.
fn measurer(options: &JsValue, family: Option<&str>) -> Rc<dyn Measure> {
    if let Some(custom) = get(options, "measurer").and_then(Custom::of) {
        return Rc::new(custom);
    }
    if string(options, "measure").as_deref() == Some("metrics") {
        return Rc::new(Metrics);
    }
    match Canvas::new(family.unwrap_or("sans-serif"), "monospace") {
        Some(canvas) => Rc::new(canvas),
        None => Rc::new(Metrics),
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
