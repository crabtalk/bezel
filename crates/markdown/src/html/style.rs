//! The stylesheet [`Html`]'s markup is set with: the document's theme as CSS.

use crate::{QuoteKind, Typography, html::Html, render};
use gpui::Hsla;
use theme::{HighlightKind, Metrics, Theme};
use ui::color::to_hex;

const ALERTS: [QuoteKind; 5] = [
    QuoteKind::Note,
    QuoteKind::Tip,
    QuoteKind::Important,
    QuoteKind::Warning,
    QuoteKind::Caution,
];

impl Html {
    /// CSS for the markup [`Html::render`] writes, inside an element of class
    /// `md`. Colors are custom properties on `.md`, `light`'s by default and
    /// `dark`'s under `prefers-color-scheme: dark`; sizes are in `px` from
    /// `typography` at the theme's base text size; fonts are `light`'s.
    pub fn stylesheet(light: &Theme, dark: &Theme, typography: &Typography) -> String {
        let mut css = String::new();
        css.push_str(".md {\n");
        colors(&mut css, light);
        css.push_str("}\n@media (prefers-color-scheme: dark) {\n  .md {\n");
        colors(&mut css, dark);
        css.push_str("  }\n}\n");

        let body = format!("\"{}\", system-ui, sans-serif", light.font_body);
        let mono = format!("\"{}\", ui-monospace, monospace", light.font_mono);
        rule(
            &mut css,
            ".md",
            &[
                ("color", "var(--md-text)".into()),
                ("font-family", body),
                ("font-size", px(typography.body.size())),
                ("line-height", typography.body.leading.to_string()),
                ("font-weight", weight(typography.body)),
            ],
        );
        for level in 1..=6 {
            let metrics = typography.heading(level);
            rule(
                &mut css,
                &format!(".md h{level}"),
                &[
                    ("font-size", px(metrics.size())),
                    ("line-height", metrics.leading.to_string()),
                    ("font-weight", weight(metrics)),
                ],
            );
        }
        rule(&mut css, ".md a", &[("color", "var(--md-accent)".into())]);
        rule(
            &mut css,
            ".md code, .md pre",
            &[
                ("font-family", mono),
                ("font-size", px(typography.code.size())),
                ("line-height", typography.code.leading.to_string()),
                ("color", "var(--md-code-text)".into()),
            ],
        );
        rule(
            &mut css,
            ".md :not(pre) > code",
            &[
                ("background", "var(--md-code-wash)".into()),
                ("border-radius", "4px".into()),
                ("padding", "0 0.25em".into()),
            ],
        );
        rule(
            &mut css,
            ".md pre",
            &[
                ("background", "var(--md-code-wash)".into()),
                ("border-radius", "8px".into()),
                ("padding", "12px 16px".into()),
                ("overflow-x", "auto".into()),
            ],
        );
        rule(
            &mut css,
            ".md blockquote",
            &[
                ("margin-left", "0".into()),
                ("padding-left", "1em".into()),
                ("border-left", "3px solid var(--md-border)".into()),
                ("color", "var(--md-text-muted)".into()),
            ],
        );
        for alert in ALERTS {
            let name = alert.label().to_lowercase();
            rule(
                &mut css,
                &format!(".md .md-alert-{name}"),
                &[("border-left-color", format!("var(--md-alert-{name})"))],
            );
            rule(
                &mut css,
                &format!(".md .md-alert-{name} > .md-alert-title"),
                &[
                    ("color", format!("var(--md-alert-{name})")),
                    ("font-weight", "600".into()),
                ],
            );
        }
        rule(
            &mut css,
            ".md hr",
            &[
                ("border", "none".into()),
                ("border-top", "1px solid var(--md-border)".into()),
            ],
        );
        rule(
            &mut css,
            ".md table",
            &[("border-collapse", "collapse".into())],
        );
        rule(
            &mut css,
            ".md th, .md td",
            &[
                ("border", "1px solid var(--md-border)".into()),
                ("padding", "4px 10px".into()),
            ],
        );
        rule(
            &mut css,
            ".md .md-tasks",
            &[("list-style", "none".into()), ("padding-left", "0".into())],
        );
        rule(
            &mut css,
            ".md .md-indent",
            &[("margin-left", "1.5em".into())],
        );
        rule(
            &mut css,
            ".md .md-image img",
            &[("max-width", "100%".into())],
        );
        rule(
            &mut css,
            ".md figcaption",
            &[
                ("color", "var(--md-text-muted)".into()),
                ("font-size", px(typography.caption.size())),
                ("line-height", typography.caption.leading.to_string()),
            ],
        );
        rule(
            &mut css,
            ".md .md-bookmark",
            &[
                ("display", "block".into()),
                ("padding", "12px 16px".into()),
                ("border", "1px solid var(--md-border)".into()),
                ("border-radius", "8px".into()),
                ("font-size", px(typography.card.size())),
            ],
        );
        for kind in HighlightKind::ALL {
            let name = kind.name();
            rule(
                &mut css,
                &format!(".md .hl-{name}"),
                &[("color", format!("var(--md-hl-{name})"))],
            );
        }
        css
    }
}

fn colors(css: &mut String, theme: &Theme) {
    let mut var = |name: &str, color: Hsla| {
        css.push_str(&format!("  --md-{name}: {};\n", to_hex(color, true)));
    };
    var("text", theme.text);
    var("text-muted", theme.text_muted);
    var("border", theme.border);
    var("accent", theme.accent);
    var("code-text", theme.code_text);
    var("code-wash", theme.code_wash);
    for alert in ALERTS {
        var(
            &format!("alert-{}", alert.label().to_lowercase()),
            render::alert_color(alert, theme),
        );
    }
    for kind in HighlightKind::ALL {
        var(&format!("hl-{}", kind.name()), theme.syntax.color(kind));
    }
}

fn rule(css: &mut String, selector: &str, declarations: &[(&str, String)]) {
    css.push_str(selector);
    css.push_str(" {\n");
    for (property, value) in declarations {
        css.push_str(&format!("  {property}: {value};\n"));
    }
    css.push_str("}\n");
}

fn px(size: f32) -> String {
    format!("{}px", (size * 100.0).round() / 100.0)
}

fn weight(metrics: Metrics) -> String {
    format!("{}", metrics.weight.0)
}
