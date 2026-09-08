//! [`Item`] — a row in a dropped menu — and [`card`], the panel that paints a
//! list of them plus the panels its [`Item::Submenu`] rows drop.
//!
//! Every menu in the system is those two over the caller's own open state: the
//! bar's dropped panel ([`crate::menubar`]), the `···` on a row, the picker
//! under a chip. The state stays with the caller because only it knows what
//! opening means — a [`crate::popover::Popup`] for one, a field for another.
//!
//! [`Cursor`] is the one piece of state the card asks for: which submenus are
//! down and which row is live. Pointer and keyboard both move it, so the two
//! can never disagree about which row an open submenu hangs off. What the
//! pointer did comes back as a [`Hit`]; acting on it stays the caller's.

use crate::{icons, popover};
use gpui::{Context, MouseDownEvent, Pixels, Point, SharedString, Window, div, prelude::*, px};
use std::{cell::Cell, rc::Rc};
use theme::{TextStyle, Theme, Typeset};

/// The leading glyph and the trailing check, at the size the rows are set in.
const GLYPH: f32 = 13.0;

/// How wide a panel sits.
const PANEL_MIN: f32 = 180.0;
/// How wide one holding a described row sits. A description is a sentence
/// rather than a name, and at the narrow width it wraps to three lines and the
/// menu reads as a paragraph with a title. One described row widens the panel,
/// the way one icon opens the glyph gutter.
const PANEL_MIN_DESCRIBED: f32 = 280.0;

/// A row in a menu.
///
/// Deliberately not a struct with an `is_separator` flag: a separator has no
/// label, no accelerator and nothing to enable, and every one of those fields
/// would have to be answered anyway.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    Action {
        label: SharedString,
        /// A second line under the label, for a row whose name does not say
        /// enough on its own — what a command will do, which file it will act
        /// on. `None` keeps the row one line tall; nothing reserves space for
        /// a description the way the glyph gutter reserves space for an icon,
        /// because a row's two lines read as one block and a blank second line
        /// would read as a gap.
        description: Option<SharedString>,
        /// The leading glyph's asset path — [`crate::icons`]' consts, or a
        /// path the app resolved at runtime. A menu where no row has one keeps
        /// no room for it.
        icon: Option<SharedString>,
        /// The accelerator to *print* — the binding itself is the app's, and
        /// bezel never dispatches it. A menu that showed a keystroke it did not
        /// own would be documenting a lie.
        keystroke: Option<SharedString>,
        /// The choice the menu is currently on, marked with a trailing check.
        checked: bool,
        enabled: bool,
    },
    /// A row that drops a menu of its own, the way a SwiftUI `Menu` nests
    /// inside a `Menu`. It carries no accelerator and nothing to check: the
    /// only thing choosing it does is open.
    Submenu {
        label: SharedString,
        icon: Option<SharedString>,
        enabled: bool,
        items: Vec<Item>,
    },
    Separator,
}

impl Item {
    pub fn action(label: impl Into<SharedString>) -> Self {
        Item::Action {
            label: label.into(),
            description: None,
            icon: None,
            keystroke: None,
            checked: false,
            enabled: true,
        }
    }

    pub fn submenu(label: impl Into<SharedString>, items: Vec<Item>) -> Self {
        Item::Submenu {
            label: label.into(),
            icon: None,
            enabled: true,
            items,
        }
    }

    /// No-ops on a separator, which has nothing to hang a glyph on.
    pub fn with_icon(mut self, icon: impl Into<SharedString>) -> Self {
        match &mut self {
            Item::Action { icon: slot, .. } | Item::Submenu { icon: slot, .. } => {
                *slot = Some(icon.into())
            }
            Item::Separator => {}
        }
        self
    }

    /// A second line under the label. No-ops on anything but an action row: a
    /// submenu's second line is the panel it opens, and a separator has no
    /// first line to put one under.
    pub fn with_description(mut self, description: impl Into<SharedString>) -> Self {
        if let Item::Action {
            description: slot, ..
        } = &mut self
        {
            *slot = Some(description.into());
        }
        self
    }

    /// No-ops on anything but an action row: a submenu's keystroke is the
    /// arrow that opens it, and a separator has nothing to hang one on.
    pub fn with_keystroke(mut self, keystroke: impl Into<SharedString>) -> Self {
        if let Item::Action {
            keystroke: slot, ..
        } = &mut self
        {
            *slot = Some(keystroke.into());
        }
        self
    }

    /// Takes the flag, because what a menu is on is decided per render. No-ops
    /// on a submenu, which is not itself a choice.
    pub fn checked(mut self, checked: bool) -> Self {
        if let Item::Action { checked: slot, .. } = &mut self {
            *slot = checked;
        }
        self
    }

    pub fn disabled(mut self) -> Self {
        match &mut self {
            Item::Action { enabled, .. } | Item::Submenu { enabled, .. } => *enabled = false,
            Item::Separator => {}
        }
        self
    }

    /// Whether the keyboard and the pointer can land here at all. A submenu
    /// with nothing to choose in it counts as unlandable: opening it would drop
    /// a panel that is a dead end.
    pub fn selectable(&self) -> bool {
        match self {
            Item::Action { enabled, .. } => *enabled,
            Item::Submenu { enabled, items, .. } => *enabled && items.iter().any(Item::selectable),
            Item::Separator => false,
        }
    }

    fn has_description(&self) -> bool {
        matches!(
            self,
            Item::Action {
                description: Some(_),
                ..
            }
        )
    }

    fn has_icon(&self) -> bool {
        matches!(
            self,
            Item::Action { icon: Some(_), .. } | Item::Submenu { icon: Some(_), .. }
        )
    }

    /// The submenu this row opens, if it is one that can be opened.
    fn opens(&self) -> Option<&[Item]> {
        match self {
            Item::Submenu {
                enabled: true,
                items,
                ..
            } => Some(items),
            _ => None,
        }
    }
}

/// The item `path` names — one row index per level, outermost first.
pub fn at<'a>(items: &'a [Item], path: &[usize]) -> Option<&'a Item> {
    let (&row, above) = path.split_last()?;
    items_at(items, above)?.get(row)
}

/// The menu `path` opens into: `items` itself for an empty path, else the rows
/// of the submenu each index names. `None` as soon as one of them does not name
/// an open-able submenu.
pub fn items_at<'a>(items: &'a [Item], path: &[usize]) -> Option<&'a [Item]> {
    let mut level = items;
    for &row in path {
        level = level.get(row)?.opens()?;
    }
    Some(level)
}

/// How many levels of `open` still name an open-able submenu — where both the
/// painting and the out-click count stop when the chain has gone stale.
fn open_depth(items: &[Item], open: &[usize]) -> usize {
    let mut level = items;
    for (depth, &row) in open.iter().enumerate() {
        match level.get(row).and_then(Item::opens) {
            Some(inner) => level = inner,
            None => return depth,
        }
    }
    open.len()
}

/// The next row the keyboard can land on, `delta` deciding the direction:
/// separators and disabled rows are stepped straight over, and both ends wrap.
/// `from` of `None` enters the menu at the edge the direction comes from.
///
/// [`popover::menu_step`] cannot do this — it counts rows and knows nothing
/// about which of them can be landed on. `None` back means *nothing* in the menu
/// is selectable, which is the one shape that would otherwise spin forever.
pub fn next_selectable(items: &[Item], from: Option<usize>, delta: isize) -> Option<usize> {
    let count = items.len();
    if count == 0 {
        return None;
    }
    let step = if delta >= 0 { 1 } else { -1 };
    let wrap = |at: usize| (at as isize + step).rem_euclid(count as isize) as usize;
    // Entering, the first candidate is the edge itself; moving, it is the row
    // after the one you are on.
    let mut at = match from {
        None if step > 0 => 0,
        None => count - 1,
        Some(at) => wrap(at.min(count - 1)),
    };
    for _ in 0..count {
        if items[at].selectable() {
            return Some(at);
        }
        at = wrap(at);
    }
    None
}

// ---------------------------------------------------------------------------
// Cursor
// ---------------------------------------------------------------------------

/// Where an open menu is being worked: `open` is the chain of submenu rows
/// currently down, outermost first, and `row` is the live row in the menu that
/// chain ends at.
///
/// One cursor for both input devices. A menu that tracked hover separately from
/// the keyboard could have two rows lit and a submenu hanging off neither.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cursor {
    open: Vec<usize>,
    row: Option<usize>,
}

impl Cursor {
    /// The submenu rows currently down, outermost first.
    pub fn open(&self) -> &[usize] {
        &self.open
    }

    /// The live row in the innermost open menu.
    pub fn row(&self) -> Option<usize> {
        self.row
    }

    /// Whether a submenu is down — where a menubar's `left` closes a level
    /// instead of crossing to the previous menu.
    pub fn nested(&self) -> bool {
        !self.open.is_empty()
    }

    /// The full path to the live row, outermost first.
    pub fn path(&self) -> Option<Vec<usize>> {
        let row = self.row?;
        let mut path = self.open.clone();
        path.push(row);
        Some(path)
    }

    /// Nothing down, nothing live — a menu opens here, and closes back to it.
    pub fn clear(&mut self) {
        self.open.clear();
        self.row = None;
    }

    /// The row lit in the panel at `depth`: the row holding the submenu open
    /// above the innermost panel, the live row in it, nothing below.
    pub fn lit(&self, depth: usize) -> Option<usize> {
        match depth.cmp(&self.open.len()) {
            std::cmp::Ordering::Less => Some(self.open[depth]),
            std::cmp::Ordering::Equal => self.row,
            std::cmp::Ordering::Greater => None,
        }
    }

    /// Move within the innermost open panel.
    pub fn step(&mut self, root: &[Item], delta: isize) {
        let Some(items) = items_at(root, &self.open) else {
            return;
        };
        self.row = next_selectable(items, self.row, delta);
    }

    /// Open the submenu under the live row and land on its first row. `false`
    /// when the live row is not one — which is a menubar's cue that `right`
    /// meant the next menu instead.
    pub fn descend(&mut self, root: &[Item]) -> bool {
        let Some(row) = self.row else { return false };
        let Some(inner) = items_at(root, &self.open)
            .and_then(|items| items.get(row))
            .and_then(Item::opens)
        else {
            return false;
        };
        self.row = next_selectable(inner, None, 1);
        self.open.push(row);
        true
    }

    /// Close the innermost submenu, landing back on the row that opened it.
    /// `false` at the top level, where closing is the whole menu's to do.
    pub fn ascend(&mut self) -> bool {
        match self.open.pop() {
            Some(row) => {
                self.row = Some(row);
                true
            }
            None => false,
        }
    }

    /// Put the cursor on the row `path` names, opening the chain above it and,
    /// if it is a submenu row, itself — pointing at a submenu row is what opens
    /// it. Nothing is lit inside the fresh panel until something moves into it.
    ///
    /// Answers whether that changed anything, because the pointer reports every
    /// move and only a change is worth a repaint.
    pub fn point_at(&mut self, root: &[Item], path: &[usize]) -> bool {
        let (open, row) = match path.split_last() {
            None => (Vec::new(), None),
            Some((&row, above)) if at(root, path).and_then(Item::opens).is_some() => {
                (above.iter().copied().chain([row]).collect(), None)
            }
            Some((&row, above)) => (above.to_vec(), Some(row)),
        };
        if self.open == open && self.row == row {
            return false;
        }
        self.open = open;
        self.row = row;
        true
    }
}

// ---------------------------------------------------------------------------
// The panels
// ---------------------------------------------------------------------------

/// What the pointer did to a menu. Every variant is a request: the caller's
/// [`Cursor`] and open state are what answer it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hit {
    /// The pointer is on this row, or a submenu row was clicked. Feed it to
    /// [`Cursor::point_at`], which is also what opens a submenu.
    Point(Vec<usize>),
    /// An action row was chosen.
    Choose(Vec<usize>),
    /// A press outside every panel of the open tree. Reported here rather than
    /// left to an `.on_mouse_down_out` on the card, which sees only its own
    /// bounds and would read a click in a submenu as a click away.
    Dismiss,
}

/// The panel a menu drops: every [`Item`] as a row, in a
/// [`popover::popover_card`], with a further panel hanging off each submenu row
/// the [`Cursor`] holds open. `id` prefixes the rows' element ids, so two menus
/// open at once keep their hover state apart.
///
/// The pointer moves the cursor rather than lighting a row of its own, so a
/// submenu can only ever hang off the row that is live. Everything the pointer
/// does arrives as a [`Hit`], dismissal included.
pub fn card<V: 'static>(
    theme: &Theme,
    id: impl Into<SharedString>,
    items: &[Item],
    cursor: &Cursor,
    cx: &mut Context<V>,
    on: impl Fn(&mut V, Hit, &mut Window, &mut Context<V>) + 'static,
) -> gpui::Div {
    let tree = Tree {
        id: id.into(),
        panels: 1 + open_depth(items, cursor.open()),
        outside: Rc::new(Cell::new((None, 0))),
        on: Rc::new(on),
    };
    tree.panel(theme, items, cursor, 0, &[], cx)
}

/// A gpui mouse listener, boxed: `Context::listener` borrows the context it is
/// made from, and these are handed back out of a method that must not.
type Listener<E> = Box<dyn Fn(&E, &mut Window, &mut gpui::App)>;

/// The caller's [`Hit`] handler, shared by every panel of one open tree.
type Reporter<V> = Rc<dyn Fn(&mut V, Hit, &mut Window, &mut Context<V>)>;

/// The parts every panel of one open tree shares.
struct Tree<V: 'static> {
    id: SharedString,
    /// How many panels will paint — the divisor of the out-click count below.
    panels: usize,
    /// The press each panel has already reported as outside itself, stamped
    /// with where it landed. A press outside *all* of them is the one that
    /// dismisses, and no panel alone can tell.
    outside: Rc<Cell<(Option<Point<Pixels>>, usize)>>,
    on: Reporter<V>,
}

impl<V: 'static> Tree<V> {
    fn panel(
        &self,
        theme: &Theme,
        items: &[Item],
        cursor: &Cursor,
        depth: usize,
        prefix: &[usize],
        cx: &mut Context<V>,
    ) -> gpui::Div {
        let lit = cursor.lit(depth);
        let down = cursor.open().get(depth).copied();
        // A menu where nothing carries a glyph keeps no room for one — a bar's
        // menus would otherwise open with an empty column down their left.
        let gutter = items.iter().any(Item::has_icon);
        let described = items.iter().any(Item::has_description);
        popover::popover_card(theme)
            .min_w(px(if described {
                PANEL_MIN_DESCRIBED
            } else {
                PANEL_MIN
            }))
            .on_mouse_down_out(self.dismissal(cx))
            .children(items.iter().enumerate().map(|(row, item)| {
                if matches!(item, Item::Separator) {
                    return popover::divider().into_any_element();
                }
                let path: Vec<usize> = prefix.iter().copied().chain([row]).collect();
                let id = row_id(&self.id, &path);
                let (label, icon, enabled) = match item {
                    Item::Action {
                        label,
                        icon,
                        enabled,
                        ..
                    }
                    | Item::Submenu {
                        label,
                        icon,
                        enabled,
                        ..
                    } => (label.clone(), icon.clone(), *enabled),
                    Item::Separator => unreachable!("separators returned above"),
                };
                let description = match item {
                    Item::Action { description, .. } => description.clone(),
                    _ => None,
                };
                let row = if enabled {
                    popover::menu_row(theme, lit == Some(row), None)
                        .id(id.clone())
                        .on_mouse_move(self.reports(Hit::Point(path.clone()), cx))
                        .on_click(self.reports(
                            match item {
                                // Clicking a submenu row opens it; there is
                                // nothing else it could mean.
                                Item::Submenu { .. } => Hit::Point(path.clone()),
                                _ => Hit::Choose(path.clone()),
                            },
                            cx,
                        ))
                } else {
                    disabled_row(theme).id(id.clone())
                };
                row.when(gutter, |row| row.child(glyph_slot(theme, icon, enabled)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(label)
                            .children(
                                description.map(|description| {
                                    description_line(theme, description, enabled)
                                }),
                            ),
                    )
                    .map(|row| match item {
                        Item::Action {
                            keystroke, checked, ..
                        } => row
                            .when(*checked, |row| {
                                row.child(
                                    icons::icon(icons::status::CHECK)
                                        .size(px(GLYPH))
                                        .text_color(theme.text),
                                )
                            })
                            .when_some(keystroke.clone(), |row, keystroke| {
                                row.child(popover::kbd_hint(theme, &keystroke))
                            }),
                        _ => row.child(
                            icons::icon(icons::arrows::ALT_ARROW_RIGHT)
                                .size(px(GLYPH))
                                .text_color(theme.text_faint),
                        ),
                    })
                    .when_some(
                        item.opens().filter(|_| down == Some(path[depth])),
                        |parent, inner| {
                            let panel = self
                                .panel(theme, inner, cursor, depth + 1, &path, cx)
                                .into_any_element();
                            parent.relative().child(popover::anchored_submenu(
                                SharedString::from(format!("{id}-panel")),
                                panel,
                            ))
                        },
                    )
                    .into_any_element()
            }))
    }

    /// One panel's share of the out-click test: it reports the press it did not
    /// contain, and whichever panel completes the tally is the one that calls
    /// it a dismissal. Order between them does not matter, only the count.
    fn dismissal(&self, cx: &mut Context<V>) -> Listener<MouseDownEvent> {
        let outside = self.outside.clone();
        let panels = self.panels;
        let on = self.on.clone();
        Box::new(
            cx.listener(move |view, event: &MouseDownEvent, window, cx| {
                let (at, count) = outside.get();
                let count = if at == Some(event.position) {
                    count + 1
                } else {
                    1
                };
                outside.set((Some(event.position), count));
                if count >= panels {
                    outside.set((None, 0));
                    on(view, Hit::Dismiss, window, cx);
                }
            }),
        )
    }

    /// A listener that hands `hit` back to the caller, whatever the event was.
    fn reports<E: 'static>(&self, hit: Hit, cx: &mut Context<V>) -> Listener<E> {
        let on = self.on.clone();
        Box::new(cx.listener(move |view, _: &E, window, cx| on(view, hit.clone(), window, cx)))
    }
}

/// A row's element id: the card's, then the path, so rows of two panels — or of
/// two menus open at once — never collide.
fn row_id(id: &SharedString, path: &[usize]) -> SharedString {
    let mut out = id.to_string();
    for row in path {
        out.push('-');
        out.push_str(&row.to_string());
    }
    SharedString::from(out)
}

/// The leading column: the row's glyph, or the room one would have taken, so a
/// menu of mixed rows keeps its labels on one edge.
fn glyph_slot(theme: &Theme, icon: Option<SharedString>, enabled: bool) -> gpui::Div {
    div()
        .flex_none()
        .size(px(GLYPH))
        .flex()
        .items_center()
        .justify_center()
        .children(icon.map(|path| {
            gpui::svg()
                .path(path)
                .size(px(GLYPH))
                .text_color(if enabled {
                    theme.text_faint
                } else {
                    theme.text_faint.opacity(0.5)
                })
        }))
}

/// The second line under a row's label: the same relationship a card row's
/// meta line has to its title ([`crate::widgets::Scaffolding::meta_line`]),
/// which is where the size and the tone come from.
///
/// It sets its own colour rather than inheriting the row's, because the row's
/// is the *label's* — a lit row paints that at full contrast, and a
/// description that followed it there would stop reading as the quieter half.
fn description_line(theme: &Theme, description: SharedString, enabled: bool) -> gpui::Div {
    div()
        .mt(px(2.0))
        .text_style(TextStyle::Subheadline)
        .text_color(if enabled {
            theme.text_muted
        } else {
            // The dimming `glyph_slot` gives a disabled glyph, so the whole
            // row fades by one rule rather than two.
            theme.text_faint.opacity(0.5)
        })
        .child(description)
}

/// A row that cannot be chosen: [`popover::menu_row`]'s metrics without its
/// hover fade or its click, because a disabled row that lit under the pointer
/// would be inviting a press that does nothing.
fn disabled_row(theme: &Theme) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.0))
        .px(px(8.0))
        .py(px(6.0))
        // The disabled twin of `popover::menu_row`, in the same card — so it
        // takes its corners from the same rule rather than a matching literal.
        .rounded(px(Theme::inset_radius(
            Theme::surface_radius(),
            popover::MENU_PAD,
        )))
        .text_style(TextStyle::Body)
        .text_color(theme.text_faint)
}
