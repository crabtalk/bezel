//! `List~int~` as `List<int>`. Pele's `diagrams/common/generics.ts`.

use crate::js::Lazy;

static RE_COMMA: Lazy = Lazy::new(r"(,)", "");

fn tildes(text: &str) -> usize {
    text.matches('~').count()
}

/// Pairs tildes from the outside in: the first with the last, the second
/// with the one before it.
fn pair_tildes(text: &str) -> String {
    let count = tildes(text);
    if count <= 1 {
        return text.to_string();
    }
    let lead = !count.is_multiple_of(2) && text.starts_with('~');
    let mut chars: Vec<char> = text.chars().collect();
    let at: Vec<usize> = chars
        .iter()
        .enumerate()
        .skip(usize::from(lead))
        .filter(|(_, c)| **c == '~')
        .map(|(i, _)| i)
        .collect();
    let (mut lo, mut hi) = (0, at.len().saturating_sub(1));
    while lo < hi {
        chars[at[lo]] = '<';
        chars[at[hi]] = '>';
        lo += 1;
        hi -= 1;
    }
    chars.into_iter().collect()
}

/// Turns `List~int~` into `List<int>`, with the quirks of Mermaid's
/// `parseGenericTypes` around commas.
pub fn parse_generic_types(input: &str) -> String {
    if !input.contains('~') {
        return input.to_string();
    }
    let sets = RE_COMMA.split_str(input);
    let mut output: Vec<String> = Vec::new();
    let mut i = 0;
    while i < sets.len() {
        let mut set = sets[i].to_string();
        if set == ","
            && i > 0
            && i + 1 < sets.len()
            && tildes(sets[i - 1]) == 1
            && tildes(sets[i + 1]) == 1
        {
            set = format!("{},{}", sets[i - 1], sets[i + 1]);
            i += 1;
            output.pop();
        }
        output.push(pair_tildes(&set));
        i += 1;
    }
    output.concat()
}
