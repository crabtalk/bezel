use ui::menubar::*;

/// `a · ─ · b(disabled) · c`
fn items() -> Vec<Item> {
    vec![
        Item::action("a"),
        Item::Separator,
        Item::action("b").disabled(),
        Item::action("c"),
    ]
}

#[test]
fn entering_lands_on_the_first_row_the_direction_meets() {
    assert_eq!(next_selectable(&items(), None, 1), Some(0));
    assert_eq!(next_selectable(&items(), None, -1), Some(3));
    // Entering downward past a leading separator.
    let leading = vec![Item::Separator, Item::action("a")];
    assert_eq!(next_selectable(&leading, None, 1), Some(1));
}

#[test]
fn stepping_skips_what_cannot_be_chosen() {
    // 0 → past the separator AND the disabled row → 3.
    assert_eq!(next_selectable(&items(), Some(0), 1), Some(3));
    assert_eq!(next_selectable(&items(), Some(3), -1), Some(0));
}

#[test]
fn both_ends_wrap() {
    assert_eq!(next_selectable(&items(), Some(3), 1), Some(0));
    assert_eq!(next_selectable(&items(), Some(0), -1), Some(3));
}

#[test]
fn a_menu_with_nothing_to_choose_answers_none() {
    // The shape that would otherwise walk the ring forever.
    let dead = vec![Item::Separator, Item::action("x").disabled()];
    assert_eq!(next_selectable(&dead, None, 1), None);
    assert_eq!(next_selectable(&dead, Some(1), -1), None);
    assert_eq!(next_selectable(&[], None, 1), None);
}

#[test]
fn one_selectable_row_is_its_own_neighbour() {
    let lone = vec![Item::Separator, Item::action("only")];
    assert_eq!(next_selectable(&lone, Some(1), 1), Some(1));
    assert_eq!(next_selectable(&lone, Some(1), -1), Some(1));
}

#[test]
fn a_separator_carries_nothing() {
    assert_eq!(Item::Separator.with_keystroke("⌘K"), Item::Separator);
    assert_eq!(Item::Separator.disabled(), Item::Separator);
    assert!(!Item::Separator.selectable());
    assert!(Item::action("a").selectable());
    assert!(!Item::action("a").disabled().selectable());
}
