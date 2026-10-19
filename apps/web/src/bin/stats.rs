//! Prints `{ "diagrams": <n>, "themes": <n> }`, the counts the home page quotes
//! that the catalog does not carry.

use theme::{BASE_COLORS, ThemeFamily};

fn main() {
    let diagrams = mermaid::Kind::ALL.len();
    // Each base color in light and dark, and each bundled family's variants.
    let variants = ThemeFamily::bundled()
        .iter()
        .map(|family| usize::from(family.dark.is_some()) + usize::from(family.light.is_some()))
        .sum::<usize>();
    let themes = BASE_COLORS.len() * 2 + variants;
    println!("{{\"diagrams\":{diagrams},\"themes\":{themes}}}");
}
