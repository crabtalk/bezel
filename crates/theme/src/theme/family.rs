//! A palette as data: a named family with a dark variant, a light variant or
//! both, each a partial set of colour tokens laid over the shipped palette for
//! its appearance.

use std::collections::BTreeMap;

use gpui::Hsla;
use serde::Deserialize;

use crate::{
    Appearance, color,
    theme::{SyntaxPalette, Theme},
};

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
/// [light.seed]
/// surface = "#fbf1c7"
/// ink = "#3c3836"
/// accent = "#076678"
/// ```
///
/// Keys are the names [`Theme::tokens_mut`] gives; a nested table joins its
/// key onto its parent's with a dot, so `[dark.syntax]` `keyword` is
/// `syntax.keyword`. Values are `#rrggbb` or `#rrggbbaa`. A `seed` table is a
/// [`Seed`], applied before the variant's tokens.
///
/// A family has at least one variant; deserializing one with neither fails.
#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "RawFamily")]
pub struct ThemeFamily {
    pub name: String,
    pub author: Option<String>,
    pub dark: Option<Variant>,
    pub light: Option<Variant>,
}

#[derive(Deserialize)]
struct RawFamily {
    name: String,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    dark: Option<Variant>,
    #[serde(default)]
    light: Option<Variant>,
}

impl TryFrom<RawFamily> for ThemeFamily {
    type Error = String;

    fn try_from(raw: RawFamily) -> Result<Self, String> {
        if raw.dark.is_none() && raw.light.is_none() {
            return Err(format!(
                "theme family `{}` has neither a dark nor a light variant",
                raw.name
            ));
        }
        Ok(Self {
            name: raw.name,
            author: raw.author,
            dark: raw.dark,
            light: raw.light,
        })
    }
}

impl ThemeFamily {
    /// The variant for `appearance`, if the family has one.
    pub fn variant(&self, appearance: Appearance) -> Option<&Variant> {
        match appearance {
            Appearance::Dark => self.dark.as_ref(),
            Appearance::Light => self.light.as_ref(),
        }
    }

    /// The shipped palette for `appearance` with this family's variant laid
    /// over it, carrying the family's name in [`Theme::family`]. `None` when
    /// the family has no variant for `appearance`.
    pub fn theme(&self, appearance: Appearance) -> Option<Theme> {
        let variant = self.variant(appearance)?;
        let mut theme = Theme::for_appearance(appearance);
        variant.apply(&mut theme);
        theme.family = Some(self.name.clone().into());
        Some(theme)
    }
}

/// What one variant sets: a [`Seed`] and the tokens laid over it. A token
/// neither sets keeps the shipped palette's value.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(from = "RawVariant")]
pub struct Variant {
    seed: Option<Seed>,
    tokens: BTreeMap<String, Hsla>,
}

#[derive(Deserialize)]
struct RawVariant {
    #[serde(default)]
    seed: Option<Seed>,
    #[serde(flatten)]
    tokens: BTreeMap<String, Node>,
}

/// A value in a variant's table: a colour, or a table of more.
#[derive(Deserialize)]
#[serde(untagged)]
enum Node {
    Color(Hsla),
    Table(BTreeMap<String, Node>),
}

impl From<RawVariant> for Variant {
    fn from(raw: RawVariant) -> Self {
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
        let mut tokens = BTreeMap::new();
        flatten("", raw.tokens, &mut tokens);
        Self {
            seed: raw.seed,
            tokens,
        }
    }
}

impl Variant {
    /// The colour this variant's tokens give `token`, if they name one. A
    /// colour the seed decides is not one.
    pub fn get(&self, token: &str) -> Option<Hsla> {
        self.tokens.get(token).copied()
    }

    pub fn seed(&self) -> Option<&Seed> {
        self.seed.as_ref()
    }

    /// Apply the seed, then write every token this variant names into
    /// `theme`. Returns the names that are not tokens, which are skipped.
    pub fn apply(&self, theme: &mut Theme) -> Vec<&str> {
        if let Some(seed) = &self.seed {
            seed.apply(theme);
        }
        let mut slots: BTreeMap<&str, &mut Hsla> = theme.tokens_mut().into_iter().collect();
        self.tokens
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

/// The few colours a whole variant is grown from: a background, the text on
/// it and an accent, with the diff and highlight colours optional.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Seed {
    /// The main panel, [`Theme::bg`].
    pub surface: Hsla,
    /// Body text, [`Theme::text`].
    pub ink: Hsla,
    pub accent: Hsla,
    /// Added lines and success. Unset keeps the shipped green.
    #[serde(default)]
    pub added: Option<Hsla>,
    /// Removed lines and danger. Unset keeps the shipped red.
    #[serde(default)]
    pub removed: Option<Hsla>,
    /// [`Theme::busy`]. Unset keeps the shipped pink.
    #[serde(default)]
    pub highlight: Option<Hsla>,
}

impl Seed {
    /// Write every opaque surface, text, accent and syntax token from the
    /// seed into `theme`, which is the shipped palette for its appearance.
    /// Translucent tokens (hovers, borders, washes) and warning keep the
    /// shipped values, as does the terminal's ANSI set.
    pub fn apply(&self, theme: &mut Theme) {
        let (bg, ink, accent) = (self.surface, self.ink, self.accent);
        let toward = |t: f32| color::mix_oklab(bg, ink, t);
        let dark = theme.appearance.is_dark();
        theme.bg = bg;
        theme.surface = toward(0.035);
        if dark {
            theme.surface_card = toward(0.04);
            theme.surface_dialog = toward(0.05);
            theme.surface_overlay = toward(0.08);
            theme.surface_raised = toward(0.10);
            theme.surface_raised_hover = toward(0.15);
        } else {
            theme.surface_card = bg;
            theme.surface_dialog = bg;
            theme.surface_overlay = bg;
            theme.surface_raised = toward(0.07);
            theme.surface_raised_hover = toward(0.12);
            theme.input_bg = bg;
        }
        theme.text = ink;
        theme.text_dim = toward(0.58);
        theme.solid = ink;
        theme.on_solid = bg;
        theme.cursor = ink;
        theme.code_text = ink;
        theme.accent = accent;
        theme.accent_strong = accent;
        theme.on_accent = if color::contrast_ratio(accent, bg) >= color::contrast_ratio(accent, ink)
        {
            bg
        } else {
            ink
        };
        theme.caret = accent;
        theme.selection = color::mix_oklab(bg, accent, if dark { 0.35 } else { 0.25 });
        if let Some(removed) = self.removed {
            theme.danger = removed;
            theme.danger_strong = removed;
            theme.danger_muted = color::mix_oklab(removed, ink, 0.3);
            theme.diff_del = removed;
        }
        if let Some(added) = self.added {
            theme.success = added;
            theme.success_muted = color::mix_oklab(added, ink, 0.3);
            theme.diff_add = added;
        }
        if let Some(highlight) = self.highlight {
            theme.busy = highlight;
        }
        theme.terminal_bg = bg;
        theme.vibrancy_tone = bg;
        let comment = toward(0.45);
        theme.syntax = if dark {
            SyntaxPalette::dark(ink, comment, theme.danger)
        } else {
            SyntaxPalette::light(ink, comment, theme.danger)
        };
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
