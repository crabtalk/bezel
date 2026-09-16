//! Regions of one language inside another.

use theme::HighlightKind;

const PAGE: &str = r#"<!DOCTYPE html>
<html>
  <style>
    .banner { color: #ff0000; }
  </style>
  <script>
    const total = 41 + 1;
  </script>
</html>
"#;

/// The kind painting `needle`, where `needle` occurs once in `source`.
fn kind_at(source: &str, needle: &str) -> Option<HighlightKind> {
    let at = source.find(needle).expect("needle is in the source");
    syntax::highlight(source, "html")?
        .into_iter()
        .find(|(range, _)| range.start <= at && at < range.end)
        .map(|(_, kind)| kind)
}

/// An html document paints its own markup.
#[test]
fn the_host_language_still_paints() {
    let spans = syntax::highlight(PAGE, "html").expect("html is carried");
    assert!(!spans.is_empty(), "the html query compiled to nothing");
    assert_eq!(kind_at(PAGE, "DOCTYPE"), Some(HighlightKind::Keyword));
}

/// A `<style>` body is painted by the CSS grammar, not left as raw text. The
/// property name is a CSS capture and has no counterpart in the html query.
#[test]
fn a_style_body_is_painted_as_css() {
    assert_eq!(kind_at(PAGE, "color"), Some(HighlightKind::Property));
}

/// A `<script>` body is painted by the TSX grammar. `const` is a keyword there
/// and nothing at all in html or css.
#[test]
fn a_script_body_is_painted_as_javascript() {
    assert_eq!(kind_at(PAGE, "const"), Some(HighlightKind::Keyword));
    assert_eq!(kind_at(PAGE, "41"), Some(HighlightKind::Number));
}

/// The directive captures drive injection and never paint. `@injection.content`
/// covers the whole `<style>` body, so recognizing it would lay one span over
/// the lot and bury the css spans underneath.
#[test]
fn injection_captures_do_not_paint() {
    let spans = syntax::highlight(PAGE, "html").expect("html is carried");
    let start = PAGE.find(".banner").expect("the rule is in the source");
    let end = PAGE.find('}').expect("the rule closes") + 1;
    assert!(
        !spans
            .iter()
            .any(|(range, _)| range.start <= start && range.end >= end),
        "a span blankets the whole css rule; an injection capture is painting"
    );
}

/// css stands on its own as well as inside html.
#[test]
fn css_is_a_language_in_its_own_right() {
    let spans = syntax::highlight(".a { color: red; }", "css").expect("css is carried");
    assert!(!spans.is_empty(), "the css query compiled to nothing");
}
