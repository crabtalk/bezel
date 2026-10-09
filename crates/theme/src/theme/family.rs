//! A palette as data: a named family with a dark and a light variant, each a
//! partial set of colour tokens laid over the shipped palette for its
//! appearance.

use std::collections::BTreeMap;

use gpui::Hsla;
use serde::Deserialize;

use crate::{Appearance, theme::Theme};

/// One theme family, as a file declares it:
///
/// ```toml
/// name = "Gruvbox"
///
/// [dark]
/// bg = "#282828"
/// text = "#ebdbb2"
///
/// [dark.syntax]
/// keyword = "#fb4934"
///
/// [light]
/// bg = "#fbf1c7"
/// ```
///
/// Keys are the names [`Theme::tokens_mut`] gives; a nested table joins its
/// key onto its parent's with a dot, so `[dark.syntax]` `keyword` is
/// `syntax.keyword`. Values are `#rrggbb` or `#rrggbbaa`.
#[derive(Debug, Clone, Deserialize)]
pub struct ThemeFamily {
    pub name: String,
    #[serde(default)]
    pub author: Option<String>,
    pub dark: Variant,
    pub light: Variant,
}

impl ThemeFamily {
    pub fn variant(&self, appearance: Appearance) -> &Variant {
        match appearance {
            Appearance::Dark => &self.dark,
            Appearance::Light => &self.light,
        }
    }

    /// The shipped palette for `appearance` with this family's variant laid
    /// over it.
    pub fn theme(&self, appearance: Appearance) -> Theme {
        let mut theme = Theme::for_appearance(appearance);
        self.variant(appearance).apply(&mut theme);
        theme
    }
}

/// The tokens one variant sets. A token it leaves out keeps the shipped
/// palette's value.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(from = "BTreeMap<String, Node>")]
pub struct Variant(BTreeMap<String, Hsla>);

/// A value in a variant's table: a colour, or a table of more.
#[derive(Deserialize)]
#[serde(untagged)]
enum Node {
    Color(Hsla),
    Table(BTreeMap<String, Node>),
}

impl From<BTreeMap<String, Node>> for Variant {
    fn from(table: BTreeMap<String, Node>) -> Self {
        fn flatten(prefix: &str, table: BTreeMap<String, Node>, out: &mut BTreeMap<String, Hsla>) {
            for (key, node) in table {
                let name = match prefix {
                    "" => key,
                    _ => format!("{prefix}.{key}"),
                };
                match node {
                    Node::Color(color) => {
                        out.insert(name, color);
                    }
                    Node::Table(table) => flatten(&name, table, out),
                }
            }
        }
        let mut out = BTreeMap::new();
        flatten("", table, &mut out);
        Self(out)
    }
}

impl Variant {
    /// The colour this variant gives `token`, if it names one.
    pub fn get(&self, token: &str) -> Option<Hsla> {
        self.0.get(token).copied()
    }

    /// Write every token this variant names into `theme`. Returns the names
    /// that are not tokens, which are skipped.
    pub fn apply(&self, theme: &mut Theme) -> Vec<&str> {
        let mut slots: BTreeMap<&str, &mut Hsla> = theme.tokens_mut().into_iter().collect();
        self.0
            .iter()
            .filter_map(|(name, color)| match slots.get_mut(name.as_str()) {
                Some(slot) => {
                    **slot = *color;
                    None
                }
                None => Some(name.as_str()),
            })
            .collect()
    }
}

impl Theme {
    /// Every colour token by the name a [`ThemeFamily`] file sets it under.
    ///
    /// `text_muted` and `text_faint` are absent: [`Brand::ink`](crate::Brand::ink)
    /// writes them from `text` on every install.
    pub fn tokens_mut(&mut self) -> Vec<(&'static str, &mut Hsla)> {
        let syntax = &mut self.syntax;
        let [
            black,
            red,
            green,
            yellow,
            blue,
            magenta,
            cyan,
            white,
            bright_black,
            bright_red,
            bright_green,
            bright_yellow,
            bright_blue,
            bright_magenta,
            bright_cyan,
            bright_white,
        ] = &mut self.terminal_ansi;
        vec![
            ("bg", &mut self.bg),
            ("surface", &mut self.surface),
            ("surface_raised", &mut self.surface_raised),
            ("surface_card", &mut self.surface_card),
            ("surface_dialog", &mut self.surface_dialog),
            ("surface_overlay", &mut self.surface_overlay),
            ("element_hover", &mut self.element_hover),
            ("element_active", &mut self.element_active),
            ("border_faint", &mut self.border_faint),
            ("border", &mut self.border),
            ("border_strong", &mut self.border_strong),
            ("text", &mut self.text),
            ("text_dim", &mut self.text_dim),
            ("solid", &mut self.solid),
            ("on_solid", &mut self.on_solid),
            ("accent", &mut self.accent),
            ("accent_strong", &mut self.accent_strong),
            ("on_accent", &mut self.on_accent),
            ("danger", &mut self.danger),
            ("danger_muted", &mut self.danger_muted),
            ("warning", &mut self.warning),
            ("warning_muted", &mut self.warning_muted),
            ("success", &mut self.success),
            ("busy", &mut self.busy),
            ("success_muted", &mut self.success_muted),
            ("surface_raised_hover", &mut self.surface_raised_hover),
            ("band", &mut self.band),
            ("input_bg", &mut self.input_bg),
            ("selection", &mut self.selection),
            ("cursor", &mut self.cursor),
            ("caret", &mut self.caret),
            ("ring", &mut self.ring),
            ("drop_line", &mut self.drop_line),
            ("drop_target", &mut self.drop_target),
            ("danger_strong", &mut self.danger_strong),
            ("code_text", &mut self.code_text),
            ("code_wash", &mut self.code_wash),
            ("diff_add", &mut self.diff_add),
            ("diff_del", &mut self.diff_del),
            ("diff_hunk_bg", &mut self.diff_hunk_bg),
            ("vibrancy_tone", &mut self.vibrancy_tone),
            ("syntax.comment", &mut syntax.comment),
            ("syntax.keyword", &mut syntax.keyword),
            ("syntax.string", &mut syntax.string),
            ("syntax.string_special", &mut syntax.string_special),
            ("syntax.escape", &mut syntax.escape),
            ("syntax.number", &mut syntax.number),
            ("syntax.boolean", &mut syntax.boolean),
            ("syntax.type_name", &mut syntax.type_name),
            ("syntax.type_builtin", &mut syntax.type_builtin),
            ("syntax.constructor", &mut syntax.constructor),
            ("syntax.function", &mut syntax.function),
            ("syntax.function_builtin", &mut syntax.function_builtin),
            ("syntax.macro_name", &mut syntax.macro_name),
            ("syntax.property", &mut syntax.property),
            ("syntax.constant", &mut syntax.constant),
            ("syntax.variable", &mut syntax.variable),
            ("syntax.variable_special", &mut syntax.variable_special),
            ("syntax.parameter", &mut syntax.parameter),
            ("syntax.operator", &mut syntax.operator),
            ("syntax.punctuation", &mut syntax.punctuation),
            ("syntax.tag", &mut syntax.tag),
            ("syntax.attribute", &mut syntax.attribute),
            ("syntax.label", &mut syntax.label),
            ("syntax.invalid", &mut syntax.invalid),
            ("terminal.bg", &mut self.terminal_bg),
            ("terminal.ansi.black", black),
            ("terminal.ansi.red", red),
            ("terminal.ansi.green", green),
            ("terminal.ansi.yellow", yellow),
            ("terminal.ansi.blue", blue),
            ("terminal.ansi.magenta", magenta),
            ("terminal.ansi.cyan", cyan),
            ("terminal.ansi.white", white),
            ("terminal.ansi.bright_black", bright_black),
            ("terminal.ansi.bright_red", bright_red),
            ("terminal.ansi.bright_green", bright_green),
            ("terminal.ansi.bright_yellow", bright_yellow),
            ("terminal.ansi.bright_blue", bright_blue),
            ("terminal.ansi.bright_magenta", bright_magenta),
            ("terminal.ansi.bright_cyan", bright_cyan),
            ("terminal.ansi.bright_white", bright_white),
        ]
    }
}
