//! The package under Node: `wasm-pack test --node crates/markdown-wasm`.

#![cfg(target_arch = "wasm32")]

use js_sys::Reflect;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::wasm_bindgen_test;

fn field(value: &JsValue, key: &str) -> JsValue {
    Reflect::get(value, &key.into()).expect("a field")
}

#[wasm_bindgen_test]
fn a_flowchart_renders_with_its_type_and_size() {
    let drawn =
        markdown_wasm::render("flowchart LR\n  A --> B", JsValue::UNDEFINED).expect("a drawing");
    let svg = field(&drawn, "svg").as_string().expect("svg");
    assert!(svg.starts_with("<svg"), "{svg}");
    assert_eq!(
        field(&drawn, "type").as_string().as_deref(),
        Some("flowchart")
    );
    assert!(
        field(&drawn, "width")
            .as_f64()
            .is_some_and(|width| width > 0.0)
    );
    assert!(js_sys::Array::is_array(&field(&drawn, "links")));
}

#[wasm_bindgen_test]
fn a_failure_throws_peles_error() {
    let thrown = markdown_wasm::render("flowchart LR\n  A -->", JsValue::UNDEFINED)
        .expect_err("source that does not parse");
    assert_eq!(
        field(&thrown, "name").as_string().as_deref(),
        Some("PeleError")
    );
    assert_eq!(
        field(&thrown, "code").as_string().as_deref(),
        Some("syntax")
    );
    assert_eq!(
        field(&thrown, "type").as_string().as_deref(),
        Some("flowchart")
    );
    assert!(
        field(&thrown, "line")
            .as_f64()
            .is_some_and(|line| line > 0.0)
    );
    assert!(field(&thrown, "snippet").as_string().is_some());
}

#[wasm_bindgen_test]
fn a_gantt_chart_reads_the_clock() {
    let drawn = markdown_wasm::render(
        "gantt\n  section A\n  Task :a1, 2024-01-01, 3d",
        JsValue::UNDEFINED,
    );
    assert!(drawn.is_ok());
}

#[wasm_bindgen_test]
fn types_are_detected_and_models_parsed() {
    assert_eq!(
        markdown_wasm::detect_type("sequenceDiagram\n  A->>B: hi").as_deref(),
        Some("sequence")
    );
    assert!(markdown_wasm::supports("pie\n  \"a\" : 1"));
    assert!(!markdown_wasm::supports("not a diagram"));
    let model = markdown_wasm::parse("pie\n  \"a\" : 1", JsValue::UNDEFINED).expect("a model");
    assert_eq!(field(&model, "type").as_string().as_deref(), Some("pie"));
}
