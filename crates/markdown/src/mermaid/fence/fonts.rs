//! The fonts a fence's diagram is measured in: the system's, as usvg's font
//! database reads them, the database gpui's SVG renderer resolves the same
//! family names in. A label is shaped with rustybuzz, as usvg shapes it, a
//! character the face lacks taken from the first face that has it.
//!
//! Loading the system's fonts reads every font file's names, a quarter of a
//! second on a Mac, once per process.

use std::{cell::RefCell, collections::HashMap, sync::LazyLock};

use usvg::fontdb::{Database, Family, ID, Query, Style as FaceStyle, Weight};

use crate::mermaid::{Measure, text::Style};

/// Families tried after the theme's sans, in order: the first the system has
/// a regular and a bold face of is drawn in.
const SANS: [&str; 9] = [
    "Helvetica Neue",
    "Segoe UI",
    "Roboto",
    "Ubuntu",
    "Cantarell",
    "Noto Sans",
    "DejaVu Sans",
    "Liberation Sans",
    "Arial",
];
/// Families tried after the theme's mono, as [`SANS`] is.
const MONO: [&str; 9] = [
    "SF Mono",
    "Menlo",
    "Consolas",
    "Cascadia Mono",
    "Ubuntu Mono",
    "DejaVu Sans Mono",
    "Liberation Mono",
    "Noto Sans Mono",
    "Courier New",
];
/// Labels' widths kept per font. Past this the cache starts over.
const KEPT: usize = 20_000;

static SYSTEM: LazyLock<Database> = LazyLock::new(|| {
    let mut db = Database::new();
    db.load_system_fonts();
    db
});

thread_local! {
    /// [`Families::resolve`] by the theme's sans and mono.
    static RESOLVED: RefCell<HashMap<(String, String), Families>> = RefCell::new(HashMap::new());
    /// The face each character a base face lacks is taken from.
    static FALLBACKS: RefCell<HashMap<(ID, char), Option<ID>>> = RefCell::new(HashMap::new());
    /// Widths in em by family, style and text.
    static WIDTHS: RefCell<HashMap<(Families, u8, String), f64>> = RefCell::new(HashMap::new());
}

/// The families a diagram is measured and drawn in.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct Families {
    pub sans: String,
    pub mono: String,
}

impl Families {
    /// The first of `sans` and then [`SANS`] the system has a regular and a
    /// bold face of, and the same for `mono`; the database's generic family
    /// where none is.
    pub(super) fn resolve(sans: &str, mono: &str) -> Self {
        let key = (sans.to_string(), mono.to_string());
        if let Some(found) = RESOLVED.with(|resolved| resolved.borrow().get(&key).cloned()) {
            return found;
        }
        let pick = |first: &str, stack: &[&str], generic: Family| {
            std::iter::once(first)
                .chain(stack.iter().copied())
                .find(|name| has_regular_and_bold(&SYSTEM, name))
                .map_or_else(|| SYSTEM.family_name(&generic).to_string(), str::to_string)
        };
        let found = Self {
            sans: pick(sans, &SANS, Family::SansSerif),
            mono: pick(mono, &MONO, Family::Monospace),
        };
        RESOLVED.with(|resolved| resolved.borrow_mut().insert(key, found.clone()));
        found
    }
}

fn has_regular_and_bold(db: &Database, name: &str) -> bool {
    let weights = || {
        db.faces()
            .filter(|face| face.style == FaceStyle::Normal)
            .filter(|face| face.families.iter().any(|(family, _)| family == name))
            .map(|face| face.weight.0)
    };
    weights().any(|weight| weight == 400) && weights().any(|weight| weight >= 600)
}

/// Labels measured in [`Families`], shaped as usvg shapes them.
pub(super) struct Shaper(pub Families);

impl Shaper {
    fn face(&self, style: u8) -> Option<ID> {
        let family = match style & Style::MONO {
            0 => &self.0.sans,
            _ => &self.0.mono,
        };
        SYSTEM.query(&Query {
            families: &[Family::Name(family)],
            weight: if style & Style::BOLD != 0 {
                Weight::BOLD
            } else {
                Weight::NORMAL
            },
            style: if style & Style::ITALIC != 0 {
                FaceStyle::Italic
            } else {
                FaceStyle::Normal
            },
            ..Query::default()
        })
    }
}

impl Measure for Shaper {
    fn width(&self, text: &str, size: f64, style: u8) -> f64 {
        let key = (self.0.clone(), style, text.to_string());
        if let Some(em) = WIDTHS.with(|widths| widths.borrow().get(&key).copied()) {
            return em * size;
        }
        let em = self.face(style).map_or(0.0, |base| em_width(text, base));
        WIDTHS.with(|widths| {
            let mut widths = widths.borrow_mut();
            if widths.len() >= KEPT {
                widths.clear();
            }
            widths.insert(key, em);
        });
        em * size
    }
}

/// `text`'s advance in em: each run of characters shaped in the first of
/// `base` and its fallbacks that has them.
fn em_width(text: &str, base: ID) -> f64 {
    let mut total = 0.0;
    let mut run_start = 0;
    let mut run_face = None;
    for (at, c) in text.char_indices() {
        let face = if has_char(base, c) {
            base
        } else {
            fallback(c, base).unwrap_or(base)
        };
        if run_face != Some(face) {
            if let Some(previous) = run_face {
                total += shaped(&text[run_start..at], previous);
            }
            run_start = at;
            run_face = Some(face);
        }
    }
    if let Some(face) = run_face {
        total += shaped(&text[run_start..], face);
    }
    total
}

/// usvg's default fallback: the first other face that has `c`, unless it
/// differs from `base` in style, weight and stretch all three.
fn fallback(c: char, base: ID) -> Option<ID> {
    if let Some(found) = FALLBACKS.with(|kept| kept.borrow().get(&(base, c)).copied()) {
        return found;
    }
    let base_face = SYSTEM.face(base)?;
    let found = SYSTEM
        .faces()
        .filter(|face| face.id != base)
        .filter(|face| {
            face.style == base_face.style
                || face.weight == base_face.weight
                || face.stretch == base_face.stretch
        })
        .find(|face| has_char(face.id, c))
        .map(|face| face.id);
    FALLBACKS.with(|kept| kept.borrow_mut().insert((base, c), found));
    found
}

fn has_char(face: ID, c: char) -> bool {
    SYSTEM
        .with_face_data(face, |data, index| {
            rustybuzz::ttf_parser::Face::parse(data, index)
                .ok()?
                .glyph_index(c)
        })
        .flatten()
        .is_some()
}

fn shaped(text: &str, face: ID) -> f64 {
    SYSTEM
        .with_face_data(face, |data, index| {
            let face = rustybuzz::Face::from_slice(data, index)?;
            let mut buffer = rustybuzz::UnicodeBuffer::new();
            buffer.push_str(text);
            let glyphs = rustybuzz::shape(&face, &[], buffer);
            let advance: i64 = glyphs
                .glyph_positions()
                .iter()
                .map(|position| i64::from(position.x_advance))
                .sum();
            Some(advance as f64 / f64::from(face.units_per_em()))
        })
        .flatten()
        .unwrap_or(0.0)
}
