//! What the parsers stand on: preprocessing, YAML, entities, labels and
//! numbers, each read as JavaScript reads them.

use mermaid::{
    Value,
    detect::{Kind, detect},
    geom::Point,
    js::parse_json,
    preprocess::{cleanup_comments, encode_entities, preprocess},
    svg::builder::{esc, num},
    text::{Host, Label, Metrics, Style, decode_entities},
    yaml::parse_yaml,
};

#[test]
fn numbers_print_to_two_decimals() {
    assert_eq!(num(0.0).to_string(), "0");
    assert_eq!(num(1.005).to_string(), "1");
    assert_eq!(num(1.5).to_string(), "1.5");
    assert_eq!(num(12.345).to_string(), "12.35");
    assert_eq!(num(-12.345).to_string(), "-12.34");
    assert_eq!(num(-0.004).to_string(), "0");
    assert_eq!(num(0.1).to_string(), "0.1");
    assert_eq!(num(0.05).to_string(), "0.05");
    assert_eq!(num(1e12).to_string(), "1000000000000");
}

#[test]
fn escaping_drops_what_xml_cannot_hold() {
    assert_eq!(esc("a<b>&\"c\u{1}"), "a&lt;b&gt;&amp;&quot;c");
    assert_eq!(esc("plain"), "plain");
}

#[test]
fn front_matter_gives_title_and_config() {
    let pre = preprocess(
        "---\ntitle: Hello\nconfig:\n  flowchart:\n    curve: linear\n---\ngraph TD\nA-->B",
    )
    .unwrap();
    assert_eq!(pre.title.as_deref(), Some("Hello"));
    assert_eq!(pre.text, "graph TD\nA-->B");
    assert_eq!(pre.line_offset, 6);
    let flowchart = pre.config["flowchart"].as_object().unwrap();
    assert_eq!(
        flowchart.get("curve"),
        Some(&Value::String("linear".into()))
    );
}

#[test]
fn bad_front_matter_is_a_syntax_error() {
    let error = preprocess("---\ntitle: [unclosed\n---\ninfo")
        .err()
        .unwrap();
    assert!(
        error.message.starts_with("Front matter is not valid YAML."),
        "{}",
        error.message
    );
}

#[test]
fn directives_merge_and_leave() {
    let pre =
        preprocess("%%{init: {'theme': 'dark', 'flowchart': {'curve': 'basis'}}}%%\ngraph LR\nA")
            .unwrap();
    assert_eq!(pre.config.get("theme"), Some(&Value::String("dark".into())));
    assert_eq!(pre.text, "graph LR\nA");
    let pre = preprocess("%%{wrap}%%\ninfo").unwrap();
    assert_eq!(pre.config.get("wrap"), Some(&Value::Bool(true)));
}

#[test]
fn comments_go_but_directives_stay() {
    assert_eq!(
        cleanup_comments("  %% note\ninfo\n%%{x}%%"),
        "info\n%%{x}%%"
    );
}

#[test]
fn html_attributes_take_single_quotes() {
    let pre = preprocess("graph TD\nA[<span class=\"x\">a</span>]").unwrap();
    assert_eq!(pre.text, "graph TD\nA[<span class='x'>a</span>]");
}

#[test]
fn types_are_detected_in_mermaid_order() {
    assert_eq!(detect("flowchart-elk TD"), Some(Kind::Flowchart));
    assert_eq!(detect("graph TD"), Some(Kind::Flowchart));
    assert_eq!(detect("  sequenceDiagram"), Some(Kind::Sequence));
    assert_eq!(detect("info"), Some(Kind::Info));
    assert_eq!(detect("ISHIKAWA-beta"), Some(Kind::Ishikawa));
    assert_eq!(detect("cynefin-beta:"), Some(Kind::Cynefin));
    assert_eq!(detect("cynefin-betax"), None);
    // Mermaid's C4 pattern anchors only its first name.
    assert_eq!(detect("x\nC4Container"), Some(Kind::C4));
    assert_eq!(detect("nothing"), None);
}

#[test]
fn entities_round_trip_through_placeholders() {
    let encoded = encode_entities("A[#quot;hi#quot; #35; #9829;]");
    assert_eq!(encoded, "A[ﬂ°quot¶ßhiﬂ°quot¶ß ﬂ°°35¶ß ﬂ°°9829¶ß]");
    assert_eq!(decode_entities(&encoded), "A[\"hi\" # ♥]");
    assert_eq!(decode_entities("&lt;&#65;&#x42;&nope;"), "<AB&nope;");
}

#[test]
fn colour_semicolons_are_dropped_before_entities() {
    assert_eq!(encode_entities("style A fill:#f9f;"), "style A fill:#f9f");
    // Only the line's last semicolon goes; `#fff;` before it reads as an
    // entity code, as it does in Mermaid.
    assert_eq!(
        encode_entities("classDef x fill:#fff;stroke:#000;"),
        "classDef x fill:ﬂ°fff¶ßstroke:#000"
    );
}

#[test]
fn yaml_reads_flow_block_and_scalars() {
    let value = parse_yaml(
        "a: 1\nb: [x, 'y z', \"q\\n\"]\nc:\n  - d: true\n  - .inf\ne: |\n  one\n  two\n",
    )
    .unwrap();
    let Value::Object(map) = value else {
        panic!("a map")
    };
    assert_eq!(map.get("a"), Some(&Value::from(1)));
    assert_eq!(
        map.get("b"),
        Some(&Value::Array(vec![
            Value::String("x".into()),
            Value::String("y z".into()),
            Value::String("q\n".into()),
        ]))
    );
    let Some(Value::Array(c)) = map.get("c") else {
        panic!("a list")
    };
    // JSON holds no infinity.
    assert_eq!(c[1], Value::Null);
    assert_eq!(map.get("e"), Some(&Value::String("one\ntwo\n".into())));
}

#[test]
fn json_reads_as_json_parse() {
    assert_eq!(
        parse_json("[1, \"\\u00e9\", null]"),
        Some(Value::Array(vec![
            Value::from(1),
            Value::String("é".into()),
            Value::Null,
        ]))
    );
    assert_eq!(parse_json("{'single': 1}"), None);
    assert_eq!(parse_json("01"), None);
}

#[test]
fn arial_widths() {
    let m = Metrics;
    // 'i' is 222 thousandths of an em in Arial, 'W' 944.
    assert_eq!(m.width("i", 1000.0, 0), 222.0);
    assert_eq!(m.width("W", 1000.0, 0), 944.0);
    assert_eq!(m.width("ab", 16.0, Style::MONO), 19.2);
    assert_eq!(m.width("中", 10.0, 0), 10.0);
    assert_eq!(m.width("\u{301}", 10.0, 0), 0.0);
}

#[test]
fn labels_wrap_at_spaces_and_break_at_br() {
    let label = Label::layout(Some("one two three four"), false, &Metrics, 16.0, 60.0, 0);
    assert!(label.lines.len() > 1);
    assert!(label.widths.iter().all(|&w| w <= 60.0));
    assert_eq!(label.line_height, 24.0);

    let label = Label::layout(Some("a<br/>b<BR>c\\nd"), false, &Metrics, 16.0, 400.0, 0);
    assert_eq!(label.lines.len(), 4);
}

#[test]
fn markdown_labels_read_emphasis() {
    let label = Label::layout(
        Some("plain **bold** _it_ snake_case"),
        true,
        &Metrics,
        16.0,
        4000.0,
        0,
    );
    let spans: Vec<(&str, u8)> = label.lines[0]
        .iter()
        .map(|span| (span.text.as_str(), span.style))
        .collect();
    assert_eq!(
        spans,
        vec![
            ("plain ", 0),
            ("bold", Style::BOLD),
            (" ", 0),
            ("it", Style::ITALIC),
            (" snake_case", 0),
        ]
    );
}

#[test]
fn a_one_span_label_is_one_text_element() {
    let label = Label::layout(Some("Hi"), false, &Metrics, 16.0, 4000.0, 0);
    assert_eq!(
        label
            .svg(Point::new(10.0, 10.0), " class=\"t\"")
            .to_string(),
        "<text class=\"t\" x=\"10\" y=\"15.6\" text-anchor=\"middle\">Hi</text>"
    );
}

#[test]
fn bold_spans_share_their_weight_on_the_element() {
    let label = Label::layout(
        Some("<b>a</b><br>b"),
        false,
        &Metrics,
        10.0,
        4000.0,
        Style::BOLD,
    );
    let svg = label.svg(Point::new(0.0, 0.0), "").to_string();
    assert!(
        svg.starts_with("<text font-weight=\"var(--_w)\" text-anchor=\"middle\""),
        "{svg}"
    );
}
