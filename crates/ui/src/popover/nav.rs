//! Pure reducers: the menu cursor, query matching and key classification.

use super::*;

/// Step the active row of a menu: wraps at both ends; `None` enters at the
/// edge matching the direction. Empty menus stay `None`.
pub fn menu_step(active: Option<usize>, count: usize, delta: isize) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let count_i = count as isize;
    let next = match active {
        None => {
            if delta >= 0 {
                0
            } else {
                count_i - 1
            }
        }
        Some(at) => (at as isize + delta).rem_euclid(count_i),
    };
    Some(next as usize)
}

/// Match rank of a label against a query: `0` prefix match, `1` substring,
/// `None` no match. Case-insensitive; an empty query matches everything at
/// rank 1 (input order preserved).
pub fn match_rank(query: &str, label: &str) -> Option<usize> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Some(1);
    }
    let label = label.to_lowercase();
    if label.starts_with(&query) {
        Some(0)
    } else if label.contains(&query) {
        Some(1)
    } else {
        None
    }
}

/// Filter + rank labels for a search query: prefix matches first, then
/// substring matches, stable within each rank. Returns indices into `labels`.
pub fn filter_indices<S: AsRef<str>>(query: &str, labels: &[S]) -> Vec<usize> {
    let mut ranked: Vec<(usize, usize)> = labels
        .iter()
        .enumerate()
        .filter_map(|(ix, label)| match_rank(query, label.as_ref()).map(|rank| (rank, ix)))
        .collect();
    ranked.sort_by_key(|&(rank, ix)| (rank, ix));
    ranked.into_iter().map(|(_, ix)| ix).collect()
}

/// The state behind a searchable list: the items, the ranked view of them, and
/// which row of that view is active. Shared by every picker — the palette, the
/// combobox — so the mapping below is written and tested once.
pub struct Filter {
    items: Vec<SharedString>,
    /// Indices into `items`, ranked by [`filter_indices`].
    filtered: Vec<usize>,
    /// Position within `filtered`, not within `items`.
    active: Option<usize>,
}

impl Filter {
    pub fn new(items: Vec<SharedString>) -> Self {
        let filtered: Vec<usize> = (0..items.len()).collect();
        let active = (!filtered.is_empty()).then_some(0);
        Self {
            items,
            filtered,
            active,
        }
    }

    pub fn items(&self) -> &[SharedString] {
        &self.items
    }

    /// The ranked view: indices into [`Self::items`], in display order.
    pub fn filtered(&self) -> &[usize] {
        &self.filtered
    }

    /// The highlighted row's position in the FILTERED view — what a renderer
    /// compares each row against.
    pub fn active(&self) -> Option<usize> {
        self.active
    }

    /// Re-rank against `query`, re-entering the list at the top: after
    /// narrowing, the best match should be one Enter away.
    pub fn refilter(&mut self, query: &str) {
        self.filtered = filter_indices(query, &self.items);
        self.active = (!self.filtered.is_empty()).then_some(0);
    }

    pub fn step(&mut self, delta: isize) {
        self.active = menu_step(self.active, self.filtered.len(), delta);
    }

    /// Put the cursor on a position in the FILTERED view — what the mouse
    /// calls as it crosses a row, so a menu never shows a mouse cursor and a
    /// keyboard cursor at once.
    pub fn set_active(&mut self, position: usize) {
        if position < self.filtered.len() {
            self.active = Some(position);
        }
    }

    /// The item confirming right now would pick — an index into
    /// [`Self::items`], never into the filtered view. Confusing the two is the
    /// defining bug of a filtered list: it only appears once a query narrows
    /// the rows, and then every selection picks the wrong thing.
    pub fn active_item(&self) -> Option<usize> {
        self.active
            .and_then(|position| self.filtered.get(position))
            .copied()
    }
}

/// Keys the pickers care about, classified from a raw keystroke.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuKey {
    Up,
    Down,
    /// Plain Enter — activate the highlighted row.
    Enter,
    /// Cmd/Ctrl+Enter — the "pick this folder" accelerator in the browser.
    ModEnter,
    Escape,
    Backspace,
    Other,
}

pub fn classify_key(key: &str, cmd: bool, ctrl: bool) -> MenuKey {
    match key {
        "up" => MenuKey::Up,
        "down" => MenuKey::Down,
        // Readline/emacs motion: ctrl-n/ctrl-p mirror ↓/↑ in every picker.
        // Safe to claim frame-wide — neither chord is a text-editing binding
        // in the palette keymaps, so they always bubble here unconsumed.
        "n" if ctrl => MenuKey::Down,
        "p" if ctrl => MenuKey::Up,
        "enter" if cmd || ctrl => MenuKey::ModEnter,
        "enter" => MenuKey::Enter,
        "escape" => MenuKey::Escape,
        "backspace" => MenuKey::Backspace,
        _ => MenuKey::Other,
    }
}
