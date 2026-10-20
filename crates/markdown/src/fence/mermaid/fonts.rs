//! The fonts a fence's diagram is measured and drawn in: [`ui::fonts`]. A
//! label is shaped with rustybuzz, as usvg shapes it, a character the face
//! lacks taken from the first face that has it.

use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{LazyLock, Mutex, PoisonError},
};

use ui::fonts::Database as Fonts;
use usvg::fontdb::{Database, Family, ID, Query, Style as FaceStyle, Weight};

use ::mermaid::{Host, text::Style};

/// Families tried after the theme's sans, in order: the first there is a
/// regular and a bold face of is drawn in, else the first there is a regular
/// face of.
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

thread_local! {
    /// [`Families::resolve`] by the fonts' generation and the theme's sans and
    /// mono.
    static RESOLVED: RefCell<HashMap<(u64, String, String), Families>> = RefCell::new(HashMap::new());
}

/// The face each character a base face lacks is taken from, by the fonts'
/// generation.
static FALLBACKS: Shared<(u64, ID, char), Option<ID>> = LazyLock::new(Mutex::default);
/// Widths in em by the fonts' generation, family, style and text.
static WIDTHS: Shared<(u64, Families, u8, String), f64> = LazyLock::new(Mutex::default);

/// A map every thread that draws reads and fills.
type Shared<K, V> = LazyLock<Mutex<HashMap<K, V>>>;

/// The families a diagram is measured and drawn in.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct Families {
    pub sans: String,
    pub mono: String,
}

impl Families {
    /// The first of `sans` and then [`SANS`] `fonts` has a regular and a bold
    /// face of, else a regular face of, and the same for `mono`; the
    /// database's generic family where none is.
    pub(super) fn resolve(sans: &str, mono: &str, fonts: &Fonts) -> Self {
        let key = (fonts.generation, sans.to_string(), mono.to_string());
        if let Some(found) = RESOLVED.with(|resolved| resolved.borrow().get(&key).cloned()) {
            return found;
        }
        let db = &*fonts.fonts;
        let pick = |first: &str, stack: &[&str], generic: Family| {
            let names = || std::iter::once(first).chain(stack.iter().copied());
            names()
                .find(|name| has_regular_and_bold(db, name))
                .or_else(|| names().find(|name| has_regular(db, name)))
                .map_or_else(|| db.family_name(&generic).to_string(), str::to_string)
        };
        let found = Self {
            sans: pick(sans, &SANS, Family::SansSerif),
            mono: pick(mono, &MONO, Family::Monospace),
        };
        RESOLVED.with(|resolved| resolved.borrow_mut().insert(key, found.clone()));
        found
    }
}

/// The weights of `name`'s upright faces.
fn weights<'a>(db: &'a Database, name: &'a str) -> impl Iterator<Item = u16> + 'a {
    db.faces()
        .filter(|face| face.style == FaceStyle::Normal)
        .filter(move |face| face.families.iter().any(|(family, _)| family == name))
        .map(|face| face.weight.0)
}

fn has_regular(db: &Database, name: &str) -> bool {
    weights(db, name).any(|weight| weight == 400)
}

fn has_regular_and_bold(db: &Database, name: &str) -> bool {
    has_regular(db, name) && weights(db, name).any(|weight| weight >= 600)
}

/// Labels measured in [`Families`] from `fonts`, shaped as usvg shapes them.
pub(super) struct Shaper {
    pub families: Families,
    pub fonts: Fonts,
}

impl Shaper {
    fn face(&self, style: u8) -> Option<ID> {
        let family = match style & Style::MONO {
            0 => &self.families.sans,
            _ => &self.families.mono,
        };
        self.fonts.fonts.query(&Query {
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

impl Host for Shaper {
    fn width(&self, text: &str, size: f64, style: u8) -> f64 {
        let key = (
            self.fonts.generation,
            self.families.clone(),
            style,
            text.to_string(),
        );
        let widths = || WIDTHS.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(em) = widths().get(&key).copied() {
            return em * size;
        }
        let em = self
            .face(style)
            .map_or(0.0, |base| em_width(text, base, &self.fonts));
        let mut widths = widths();
        if widths.len() >= KEPT {
            widths.clear();
        }
        widths.insert(key, em);
        em * size
    }
}

/// `text`'s advance in em: each run of characters shaped in the first of
/// `base` and its fallbacks that has them.
fn em_width(text: &str, base: ID, fonts: &Fonts) -> f64 {
    let db = &*fonts.fonts;
    let mut total = 0.0;
    let mut run_start = 0;
    let mut run_face = None;
    for (at, c) in text.char_indices() {
        let face = if has_char(db, base, c) {
            base
        } else {
            fallback(c, base, fonts).unwrap_or(base)
        };
        if run_face != Some(face) {
            if let Some(previous) = run_face {
                total += shaped(db, &text[run_start..at], previous);
            }
            run_start = at;
            run_face = Some(face);
        }
    }
    if let Some(face) = run_face {
        total += shaped(db, &text[run_start..], face);
    }
    total
}

/// usvg's default fallback: the first other face that has `c`, unless it
/// differs from `base` in style, weight and stretch all three.
fn fallback(c: char, base: ID, fonts: &Fonts) -> Option<ID> {
    let db = &*fonts.fonts;
    let key = (fonts.generation, base, c);
    let kept = || FALLBACKS.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(found) = kept().get(&key).copied() {
        return found;
    }
    let base_face = db.face(base)?;
    let found = db
        .faces()
        .filter(|face| face.id != base)
        .filter(|face| {
            face.style == base_face.style
                || face.weight == base_face.weight
                || face.stretch == base_face.stretch
        })
        .find(|face| has_char(db, face.id, c))
        .map(|face| face.id);
    kept().insert(key, found);
    found
}

fn has_char(db: &Database, face: ID, c: char) -> bool {
    db.with_face_data(face, |data, index| {
        rustybuzz::ttf_parser::Face::parse(data, index)
            .ok()?
            .glyph_index(c)
    })
    .flatten()
    .is_some()
}

fn shaped(db: &Database, text: &str, face: ID) -> f64 {
    db.with_face_data(face, |data, index| {
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
