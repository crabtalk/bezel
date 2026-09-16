//! Regions of one language inside another.

mod fixture;

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

/// The kind painting the first occurrence of `needle`.
fn kind_at(needle: &str) -> Option<HighlightKind> {
    let at = PAGE.find(needle).expect("needle is in the source");
    syntax::highlight(PAGE, "html")?
        .into_iter()
        .find(|(range, _)| range.start <= at && at < range.end)
        .map(|(_, kind)| kind)
}

/// The host language paints its own markup.
#[test]
fn the_host_language_still_paints() {
    fixture::install();
    let spans = syntax::highlight(PAGE, "html").expect("html was registered");
    assert!(!spans.is_empty(), "the html query compiled to nothing");
    assert_eq!(kind_at("DOCTYPE"), Some(HighlightKind::Keyword));
}

/// A `<style>` body is painted by the css grammar rather than left as raw text.
/// `color` is a css property name and has no counterpart in the html query.
#[test]
fn a_style_body_is_painted_as_css() {
    fixture::install();
    assert_eq!(kind_at("color"), Some(HighlightKind::Property));
    assert_eq!(kind_at("#ff0000"), Some(HighlightKind::Constant));
}

/// An injection naming a language the registry cannot paint leaves that region
/// alone. Nothing here registers `tsx`, so the `<script>` body stays plain —
/// which is why acquiring a grammar means acquiring its injection closure too.
#[test]
fn an_injection_to_an_unregistered_language_paints_nothing() {
    fixture::install();
    assert_eq!(kind_at("const"), None);
    assert_eq!(kind_at("41"), None);
}

/// The directive captures drive injection and never paint. `@injection.content`
/// covers the whole `<style>` body, so recognizing it would lay one span over
/// the lot and bury the css spans underneath.
#[test]
fn injection_captures_do_not_paint() {
    fixture::install();
    let spans = syntax::highlight(PAGE, "html").expect("html was registered");
    let start = PAGE.find(".banner").expect("the rule is in the source");
    let end = PAGE.find('}').expect("the rule closes") + 1;
    assert!(
        !spans
            .iter()
            .any(|(range, _)| range.start <= start && range.end >= end),
        "a span blankets the whole css rule; an injection capture is painting"
    );
}

/// The injected language stands on its own as well as inside a host.
#[test]
fn css_is_a_language_in_its_own_right() {
    fixture::install();
    let spans = syntax::highlight(".a { color: red; }", "css").expect("css was registered");
    assert!(!spans.is_empty(), "the css query compiled to nothing");
}
