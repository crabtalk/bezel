use ui::menu::*;

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
fn a_description_is_an_action_rows_second_line() {
    let described = Item::action("Open…").with_description("Choose a file to edit");
    assert!(
        matches!(&described, Item::Action { description: Some(copy), .. } if copy == "Choose a file to edit")
    );

    // Nothing to put one under: a submenu's second line is the panel it opens.
    assert_eq!(Item::Separator.with_description("x"), Item::Separator);
    let submenu = Item::submenu("Open Recent", vec![Item::action("a")]);
    assert_eq!(submenu.clone().with_description("x"), submenu);
}

#[test]
fn the_builders_leave_each_others_fields_alone() {
    // Every builder rewrites the row it is given, so one that reached for the
    // wrong field would silently drop what an earlier call had put there.
    let full = Item::action("Save")
        .with_icon("icon.svg")
        .with_keystroke("⌘S")
        .with_description("Write the file to disk")
        .checked(true)
        .disabled();
    assert!(matches!(
        &full,
        Item::Action {
            icon: Some(_),
            keystroke: Some(_),
            description: Some(_),
            checked: true,
            enabled: false,
            ..
        }
    ));
    assert!(!full.selectable(), "and disabled still means disabled");
}

#[test]
fn a_separator_carries_nothing() {
    assert_eq!(Item::Separator.with_keystroke("⌘K"), Item::Separator);
    assert_eq!(Item::Separator.disabled(), Item::Separator);
    assert!(!Item::Separator.selectable());
    assert!(Item::action("a").selectable());
    assert!(!Item::action("a").disabled().selectable());
}

/// `a · Copy As › (Text · More › (Base64)) · ─ · b(disabled) · Empty ›() · c`
fn nested() -> Vec<Item> {
    vec![
        Item::action("a"),
        Item::submenu(
            "Copy As",
            vec![
                Item::action("Text"),
                Item::submenu("More", vec![Item::action("Base64")]),
            ],
        ),
        Item::Separator,
        Item::action("b").disabled(),
        Item::submenu("Empty", vec![]),
        Item::action("c"),
    ]
}

#[test]
fn a_path_names_one_row_at_any_depth() {
    let items = nested();
    assert_eq!(at(&items, &[0]), Some(&Item::action("a")));
    assert_eq!(at(&items, &[1, 0]), Some(&Item::action("Text")));
    assert_eq!(at(&items, &[1, 1, 0]), Some(&Item::action("Base64")));
    // An index that runs off a level, and a level that is not a submenu.
    assert_eq!(at(&items, &[1, 9]), None);
    assert_eq!(at(&items, &[0, 0]), None);
    assert_eq!(at(&items, &[]), None);
    // The empty path is the menu itself.
    assert_eq!(items_at(&items, &[]).map(<[Item]>::len), Some(6));
    assert_eq!(items_at(&items, &[1, 1]).map(<[Item]>::len), Some(1));
}

#[test]
fn a_submenu_with_nothing_in_it_cannot_be_landed_on() {
    let items = nested();
    assert!(items[1].selectable());
    assert!(!items[4].selectable());
    assert!(
        !Item::submenu("x", vec![Item::action("y")])
            .disabled()
            .selectable()
    );
    // …and the keyboard steps over it like any other dead row: 1 → 5, not 4.
    assert_eq!(next_selectable(&items, Some(1), 1), Some(5));
}

#[test]
fn pointing_at_a_submenu_row_is_what_opens_it() {
    let items = nested();
    let mut cursor = Cursor::default();

    assert!(cursor.point_at(&items, &[0]));
    assert_eq!(cursor.open(), &[] as &[usize]);
    assert_eq!(cursor.row(), Some(0));
    // Reporting the same row again is not a repaint.
    assert!(!cursor.point_at(&items, &[0]));

    // The submenu row goes down instead of lighting, and nothing inside the
    // fresh panel is live until the pointer moves into it.
    assert!(cursor.point_at(&items, &[1]));
    assert_eq!(cursor.open(), &[1]);
    assert_eq!(cursor.row(), None);
    assert_eq!(cursor.path(), None);
    assert!(cursor.nested());

    // A row inside it, then the sibling that closes the whole chain again.
    assert!(cursor.point_at(&items, &[1, 0]));
    assert_eq!(cursor.path(), Some(vec![1, 0]));
    assert!(cursor.point_at(&items, &[0]));
    assert!(!cursor.nested());
}

#[test]
fn the_arrows_walk_levels() {
    let items = nested();
    let mut cursor = Cursor::default();

    // Nothing live, nothing to descend into.
    assert!(!cursor.descend(&items));
    assert!(!cursor.ascend());

    cursor.step(&items, 1);
    assert_eq!(cursor.row(), Some(0));
    // An action row is not a level.
    assert!(!cursor.descend(&items));

    cursor.step(&items, 1);
    assert_eq!(cursor.row(), Some(1));
    assert!(cursor.descend(&items));
    assert_eq!(cursor.open(), &[1]);
    // Descending lands on the first row the submenu has, unlike hovering.
    assert_eq!(cursor.row(), Some(0));

    // Two levels down, then all the way back out to where it started.
    cursor.step(&items, 1);
    assert!(cursor.descend(&items));
    assert_eq!(cursor.path(), Some(vec![1, 1, 0]));
    assert!(cursor.ascend());
    assert_eq!(cursor.path(), Some(vec![1, 1]));
    assert!(cursor.ascend());
    assert_eq!(cursor.path(), Some(vec![1]));
    assert!(!cursor.ascend());
}

#[test]
fn each_panel_lights_the_row_below_it() {
    let items = nested();
    let mut cursor = Cursor::default();
    cursor.point_at(&items, &[1, 1, 0]);
    // The two rows holding the chain open, then the live one, then nothing.
    assert_eq!(cursor.lit(0), Some(1));
    assert_eq!(cursor.lit(1), Some(1));
    assert_eq!(cursor.lit(2), Some(0));
    assert_eq!(cursor.lit(3), None);
}

#[test]
fn a_chain_that_no_longer_resolves_moves_nothing() {
    // The menus are re-shaped every render, so a cursor can outlive its rows.
    let mut cursor = Cursor::default();
    cursor.point_at(&nested(), &[1, 1, 0]);
    let flat = items();
    cursor.step(&flat, 1);
    assert_eq!(cursor.row(), Some(0));
    assert!(!cursor.descend(&flat));
}
