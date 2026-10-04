//! Chips: ranges of a field's content painted as a glyph and a title.

use icons::Icon;

use super::*;

/// A range of a field's content painted as `glyph` and `title` in place of its
/// text, and edited as one unit — see [`TextField::set_chips`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chip {
    pub range: Range<usize>,
    pub glyph: Icon,
    pub title: SharedString,
}

/// What stands in a chip's glyph slot. An em space is as wide as the type is
/// tall.
const SLOT: &str = "\u{2003}";

/// A field's content with every chip replaced by what it shows: the glyph's
/// slot, a space, the title.
pub(super) struct Chipped {
    pub text: SharedString,
    pub shown: Shown,
    /// Each chip's glyph slot, in shown offsets.
    pub slots: Vec<(Range<usize>, Icon)>,
    /// Each chip's title, in shown offsets.
    pub titles: Vec<Range<usize>>,
}

impl Chipped {
    /// `text` shown as it is.
    pub fn plain(text: SharedString) -> Self {
        Self {
            text,
            shown: Shown::default(),
            slots: Vec::new(),
            titles: Vec::new(),
        }
    }
}

/// `content` with `chips` shown. `chips` is in document order; a chip that
/// overlaps the one before it, is empty, runs past the end or cuts a character
/// is left as text.
pub(super) fn chipped(content: &SharedString, chips: &[Chip]) -> Chipped {
    if chips.is_empty() {
        return Chipped::plain(content.clone());
    }
    let mut text = String::with_capacity(content.len());
    let (mut slots, mut titles, mut pairs) = (Vec::new(), Vec::new(), Vec::new());
    let mut at = 0;
    for chip in chips {
        let range = &chip.range;
        if range.start < at
            || range.is_empty()
            || range.end > content.len()
            || !content.is_char_boundary(range.start)
            || !content.is_char_boundary(range.end)
        {
            continue;
        }
        text.push_str(&content[at..range.start]);
        let from = text.len();
        text.push_str(SLOT);
        slots.push((from..text.len(), chip.glyph.clone()));
        text.push(' ');
        let title = text.len();
        // Shaping breaks lines at `\n`, and a chip is one unit on one line.
        text.push_str(&chip.title.replace('\n', " "));
        titles.push(title..text.len());
        pairs.push((range.clone(), from..text.len()));
        at = range.end;
    }
    text.push_str(&content[at..]);
    Chipped {
        text: text.into(),
        shown: Shown::new(pairs),
        slots,
        titles,
    }
}

/// Where a chip's range lands after `edit`. An edit reaching inside the chip
/// takes it, so typing next to one never grows it.
pub(super) fn follow(edit: Edit, range: &Range<usize>) -> Option<Range<usize>> {
    if edit.start < range.end && range.start < edit.old_end {
        return None;
    }
    edit.map(range.clone())
}

/// `offset` moved out of the chip it falls inside: to the chip's start when
/// `back`, its end otherwise.
pub(super) fn outside(chips: &[Chip], offset: usize, back: bool) -> usize {
    match chips
        .iter()
        .find(|chip| chip.range.start < offset && offset < chip.range.end)
    {
        Some(chip) if back => chip.range.start,
        Some(chip) => chip.range.end,
        None => offset,
    }
}

/// `range` grown to take whole every chip it cuts.
pub(super) fn widen(chips: &[Chip], range: Range<usize>) -> Range<usize> {
    outside(chips, range.start, true)..outside(chips, range.end, false)
}
