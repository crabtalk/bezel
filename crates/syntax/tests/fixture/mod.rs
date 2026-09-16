//! Two languages built the way a provider builds them, for tests that need a
//! grammar. This crate links none of its own.

use syntax::{
    lang::{Grammar, Lang},
    registry::{self, Entry},
};

const CSS_QUERY: &str = r"
(comment) @comment
(class_name) @type
(property_name) @property
(color_value) @constant.builtin
(integer_value) @number
(plain_value) @variable
";

const HTML_QUERY: &str = r#"
(tag_name) @tag
(attribute_name) @tag.attribute
(comment) @comment
(doctype) @keyword.directive
(quoted_attribute_value) @string
"=" @operator
"#;

/// A `<script>` body is TSX and a `<style>` body is CSS. Both names resolve
/// through the registry, so both must be registered for either to paint.
const HTML_INJECTIONS: &str = r#"
(script_element
  (raw_text) @injection.content
  (#set! injection.language "tsx"))

(style_element
  (raw_text) @injection.content
  (#set! injection.language "css"))
"#;

pub static CSS: Lang = Lang::new(
    "css",
    &["css"],
    Grammar::Native(tree_sitter_css::LANGUAGE),
    CSS_QUERY,
);

pub static HTML: Lang = Lang::new(
    "html",
    &["html", "htm"],
    Grammar::Native(tree_sitter_html::LANGUAGE),
    HTML_QUERY,
)
.with_injections(HTML_INJECTIONS);

/// Put both into the registry, replacing the name-only rows seeded there.
pub fn install() {
    registry::register(Entry {
        name: CSS.name,
        aliases: CSS.aliases,
        files: &["css", "scss"],
        lang: Some(&CSS),
    });
    registry::register(Entry {
        name: HTML.name,
        aliases: HTML.aliases,
        files: &["htm", "html"],
        lang: Some(&HTML),
    });
}
