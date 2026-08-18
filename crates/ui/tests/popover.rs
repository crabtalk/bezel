use gpui::SharedString;

use ui::popover::*;

#[test]
fn trigger_press_note_distinguishes_dismiss_from_open() {
    let mut popup: Popup<u8> = Popup::default();

    // Fresh open: press finds nothing mounted → click opens.
    popup.note_trigger_press();
    assert!(!popup.take_press_was_open());
    popup.open(1);

    // Trigger click while open: the card's mouse-down-out begins the
    // close on the press (either handler order) — the note still reads
    // mounted, so the click must NOT reopen.
    popup.note_trigger_press();
    popup.begin_close();
    assert!(popup.take_press_was_open());
    // Out-handler first, trigger note second: mid-exit still counts.
    popup.open(1);
    popup.begin_close();
    popup.note_trigger_press();
    assert!(popup.take_press_was_open());

    // The note is consumed — a later click starts clean.
    assert!(!popup.take_press_was_open());

    // Kind-keyed popups: a press on a DIFFERENT trigger doesn't count,
    // so that click switches menus instead of swallowing.
    let mut popup: Popup<u8> = Popup::default();
    popup.open(1);
    popup.note_trigger_press_matching(|kind| *kind == 2);
    assert!(!popup.take_press_was_open());
    popup.note_trigger_press_matching(|kind| *kind == 1);
    assert!(popup.take_press_was_open());
}

#[test]
fn menu_step_wraps_and_enters() {
    // Entering an empty menu stays out.
    assert_eq!(menu_step(None, 0, 1), None);
    assert_eq!(menu_step(Some(3), 0, 1), None);
    // Entering from nothing lands on the matching edge.
    assert_eq!(menu_step(None, 3, 1), Some(0));
    assert_eq!(menu_step(None, 3, -1), Some(2));
    // Stepping wraps both ways.
    assert_eq!(menu_step(Some(2), 3, 1), Some(0));
    assert_eq!(menu_step(Some(0), 3, -1), Some(2));
    assert_eq!(menu_step(Some(1), 3, 1), Some(2));
}

#[test]
fn filter_ranks_prefix_before_substring() {
    let labels = ["main", "feature/main-sync", "master", "dev"];
    // Prefix matches ("main", "master") come before the substring match.
    assert_eq!(filter_indices("ma", &labels), vec![0, 2, 1]);
    // Case-insensitive.
    assert_eq!(filter_indices("MA", &labels), vec![0, 2, 1]);
    // No matches → empty.
    assert!(filter_indices("zzz", &labels).is_empty());
    // Empty / whitespace query keeps input order.
    assert_eq!(filter_indices("", &labels), vec![0, 1, 2, 3]);
    assert_eq!(filter_indices("   ", &labels), vec![0, 1, 2, 3]);
}

/// Mapping the active row back through the filtered view is the whole
/// reason [`Filter`] exists: get it wrong and every selection picks the
/// wrong item — but only once a query has narrowed the rows.
#[test]
fn filter_maps_the_active_row_back_to_its_item() {
    let mut filter = Filter::new(
        ["main", "feature/main-sync", "master", "dev"]
            .iter()
            .map(|s| SharedString::from(*s))
            .collect(),
    );
    assert_eq!(filter.active_item(), Some(0), "enters at the top");

    // "ma" keeps 0, 2, 1 in rank order; row 1 of the view is item 2.
    filter.refilter("ma");
    assert_eq!(filter.filtered(), &[0, 2, 1]);
    filter.step(1);
    assert_eq!(filter.active(), Some(1), "position in the view");
    assert_eq!(filter.active_item(), Some(2), "not 1");

    // Narrowing re-enters at the top, so the best match is one Enter away.
    filter.refilter("dev");
    assert_eq!(filter.active(), Some(0));
    assert_eq!(filter.active_item(), Some(3));

    // Nothing matches: nothing to confirm, and stepping stays out.
    filter.refilter("zzz");
    assert_eq!(filter.active(), None);
    assert_eq!(filter.active_item(), None);
    filter.step(1);
    assert_eq!(filter.active_item(), None);
}

#[test]
fn empty_filter_is_inert() {
    let mut filter = Filter::new(vec![]);
    assert_eq!(filter.active(), None);
    filter.step(1);
    assert_eq!(filter.active_item(), None);
    filter.refilter("anything");
    assert_eq!(filter.active_item(), None);
}

#[test]
fn match_rank_kinds() {
    assert_eq!(match_rank("re", "release"), Some(0));
    assert_eq!(match_rank("lease", "release"), Some(1));
    assert_eq!(match_rank("x", "release"), None);
    assert_eq!(match_rank("", "anything"), Some(1));
}

#[test]
fn key_classification() {
    assert_eq!(classify_key("up", false, false), MenuKey::Up);
    assert_eq!(classify_key("down", false, false), MenuKey::Down);
    assert_eq!(classify_key("enter", false, false), MenuKey::Enter);
    assert_eq!(classify_key("enter", true, false), MenuKey::ModEnter);
    assert_eq!(classify_key("enter", false, true), MenuKey::ModEnter);
    assert_eq!(classify_key("escape", false, false), MenuKey::Escape);
    assert_eq!(classify_key("backspace", false, false), MenuKey::Backspace);
    assert_eq!(classify_key("a", false, false), MenuKey::Other);
    // Readline motion — only with ctrl held.
    assert_eq!(classify_key("n", false, true), MenuKey::Down);
    assert_eq!(classify_key("p", false, true), MenuKey::Up);
    assert_eq!(classify_key("n", false, false), MenuKey::Other);
    assert_eq!(classify_key("p", true, false), MenuKey::Other);
}

#[test]
fn tracked_upper_spaces_letters() {
    assert_eq!(tracked_upper("ab"), "A\u{200A}B");
    assert_eq!(
        tracked_upper("Question"),
        "Q\u{200A}U\u{200A}E\u{200A}S\u{200A}T\u{200A}I\u{200A}O\u{200A}N"
    );
    assert_eq!(tracked_upper(""), "");
}

#[test]
fn loadable_accessors() {
    let l: Loadable<u32> = Loadable::Ready(7);
    assert_eq!(l.ready(), Some(&7));
    assert!(!l.is_loading());
    let e: Loadable<u32> = Loadable::Error("boom".into());
    assert_eq!(e.error(), Some("boom"));
    assert!(Loadable::<u32>::Loading.is_loading());
    assert_eq!(Loadable::<u32>::default(), Loadable::Idle);
}
