use gpui::Hsla;

use crate::color;

/// Paint-only syntax colors. The hues follow the Git history graph's lane
/// palette (indigo, pink, emerald, amber, red, neutral), while light-mode
/// variants are darkened enough to remain readable as text on white.
#[derive(Debug, Clone)]
pub struct SyntaxPalette {
    pub comment: Hsla,
    pub keyword: Hsla,
    pub string: Hsla,
    pub string_special: Hsla,
    pub escape: Hsla,
    pub number: Hsla,
    pub boolean: Hsla,
    pub type_name: Hsla,
    pub type_builtin: Hsla,
    pub constructor: Hsla,
    pub function: Hsla,
    pub function_builtin: Hsla,
    pub macro_name: Hsla,
    pub property: Hsla,
    pub constant: Hsla,
    pub variable: Hsla,
    pub variable_special: Hsla,
    pub parameter: Hsla,
    pub operator: Hsla,
    pub punctuation: Hsla,
    pub tag: Hsla,
    pub attribute: Hsla,
    pub label: Hsla,
    pub invalid: Hsla,
}

impl SyntaxPalette {
    pub(crate) fn dark(text: Hsla, comment: Hsla, danger: Hsla) -> Self {
        // Same sources and 72% saturation treatment as history::graph_color.
        let indigo = git_graph_tone(color::oklch(0.673, 0.182, 276.935));
        let pink = git_graph_tone(color::oklch(0.718, 0.202, 349.761));
        let emerald = git_graph_tone(color::oklch(0.765, 0.177, 163.223));
        let amber = git_graph_tone(color::oklch(0.828, 0.189, 84.429));
        let red = git_graph_tone(danger);
        Self {
            comment,
            keyword: indigo,
            string: emerald,
            string_special: pink,
            escape: pink,
            number: amber,
            boolean: amber,
            type_name: amber,
            type_builtin: emerald,
            constructor: amber,
            function: indigo,
            function_builtin: pink,
            macro_name: pink,
            property: amber,
            constant: emerald,
            variable: text,
            variable_special: pink,
            parameter: text,
            operator: text,
            punctuation: text,
            tag: pink,
            attribute: amber,
            label: amber,
            invalid: red,
        }
    }

    pub(crate) fn light(text: Hsla, comment: Hsla, danger: Hsla) -> Self {
        // Match the light graph's hue families at text-safe lightness.
        let indigo = git_graph_tone(color::oklch(0.47, 0.20, 276.966));
        let pink = git_graph_tone(color::oklch(0.47, 0.17, 0.584));
        let emerald = git_graph_tone(color::oklch(0.46, 0.11, 163.225));
        let amber = git_graph_tone(color::oklch(0.47, 0.12, 48.998));
        let red = git_graph_tone(danger);
        Self {
            comment,
            keyword: indigo,
            string: emerald,
            string_special: pink,
            escape: pink,
            number: amber,
            boolean: amber,
            type_name: amber,
            type_builtin: emerald,
            constructor: amber,
            function: indigo,
            function_builtin: pink,
            macro_name: pink,
            property: amber,
            constant: emerald,
            variable: text,
            variable_special: pink,
            parameter: text,
            operator: text,
            punctuation: text,
            tag: pink,
            attribute: amber,
            label: amber,
            invalid: red,
        }
    }
}

/// Git history intentionally softens lane saturation so the graph remains
/// colorful without competing with content. Syntax uses the same treatment.
fn git_graph_tone(mut color: Hsla) -> Hsla {
    color.s *= 0.72;
    color
}
