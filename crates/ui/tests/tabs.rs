use ui::tabs::Strip;

fn strip(tabs: &[&str]) -> Strip<String> {
    tabs.iter().map(|tab| tab.to_string()).collect()
}

fn order(strip: &Strip<String>) -> String {
    strip.tabs().join(" ")
}

fn front(strip: &Strip<String>) -> Option<&str> {
    strip.active().map(String::as_str)
}

#[test]
fn a_run_opens_on_its_first_tab() {
    let strip = strip(&["review", "terminal", "main.rs"]);
    assert_eq!(order(&strip), "review terminal main.rs");
    assert_eq!(front(&strip), Some("review"));

    let empty: Strip<String> = Strip::new();
    assert_eq!(front(&empty), None);
    assert!(empty.is_empty());
}

#[test]
fn opening_a_tab_already_held_keeps_its_place() {
    let mut strip = strip(&["review", "terminal", "main.rs"]);
    strip.open("review".into());
    assert_eq!(order(&strip), "review terminal main.rs");
    assert_eq!(front(&strip), Some("review"));

    strip.open("lib.rs".into());
    assert_eq!(order(&strip), "review terminal main.rs lib.rs");
    assert_eq!(front(&strip), Some("lib.rs"));
}

#[test]
fn closing_the_front_tab_hands_over_to_the_right() {
    let mut strip = strip(&["review", "terminal", "main.rs"]);
    strip.activate(&"terminal".into());
    assert!(strip.close(&"terminal".into()));
    assert_eq!(order(&strip), "review main.rs");
    assert_eq!(front(&strip), Some("main.rs"));
}

#[test]
fn closing_the_last_tab_falls_back_to_the_left() {
    let mut strip = strip(&["review", "terminal"]);
    strip.activate(&"terminal".into());
    strip.close(&"terminal".into());
    assert_eq!(front(&strip), Some("review"));

    strip.close(&"review".into());
    assert_eq!(front(&strip), None);
    assert!(strip.is_empty());
}

#[test]
fn closing_a_background_tab_leaves_the_front_alone() {
    let mut strip = strip(&["review", "terminal", "main.rs"]);
    strip.activate(&"main.rs".into());
    strip.close(&"review".into());
    assert_eq!(order(&strip), "terminal main.rs");
    assert_eq!(front(&strip), Some("main.rs"));
}

#[test]
fn a_tab_that_is_not_held_is_not_closed_or_activated() {
    let mut strip = strip(&["review"]);
    assert!(!strip.close(&"absent".into()));
    assert!(!strip.activate(&"absent".into()));
    assert_eq!(order(&strip), "review");
    assert_eq!(front(&strip), Some("review"));
}

#[test]
fn cycling_wraps_at_both_ends() {
    let mut strip = strip(&["a", "b", "c"]);
    strip.cycle(1);
    assert_eq!(front(&strip), Some("b"));
    strip.cycle(1);
    strip.cycle(1);
    assert_eq!(front(&strip), Some("a"));
    strip.cycle(-1);
    assert_eq!(front(&strip), Some("c"));
}

#[test]
fn a_strip_with_tabs_always_has_one_in_front() {
    let mut strip = strip(&["a", "b", "c"]);
    for tab in ["b", "a", "c"] {
        strip.activate(&tab.into());
        strip.close(&tab.into());
        assert_eq!(strip.active().is_some(), !strip.is_empty());
    }
    assert_eq!(front(&strip), None);
}

#[test]
fn a_lone_tab_cycles_to_itself() {
    let mut strip = strip(&["only"]);
    strip.cycle(1);
    assert_eq!(front(&strip), Some("only"));
    strip.cycle(-1);
    assert_eq!(front(&strip), Some("only"));
}

#[test]
fn an_empty_strip_does_not_cycle() {
    let mut strip: Strip<String> = Strip::new();
    strip.cycle(1);
    assert_eq!(front(&strip), None);
}

#[test]
fn reordering_moves_the_tab_and_not_the_front() {
    let mut strip = strip(&["a", "b", "c"]);
    strip.activate(&"a".into());
    strip.reorder(0, 2);
    assert_eq!(order(&strip), "b c a");
    assert_eq!(front(&strip), Some("a"));

    strip.reorder(2, 0);
    assert_eq!(order(&strip), "a b c");
}

#[test]
fn a_reorder_off_the_end_does_nothing() {
    let mut strip = strip(&["a", "b", "c"]);
    strip.reorder(0, 9);
    strip.reorder(9, 0);
    strip.reorder(1, 1);
    assert_eq!(order(&strip), "a b c");
}

#[test]
fn index_of_reports_where_a_tab_sits() {
    let strip = strip(&["a", "b", "c"]);
    assert_eq!(strip.index_of(&"b".into()), Some(1));
    assert_eq!(strip.index_of(&"absent".into()), None);
    assert_eq!(strip.len(), 3);
    assert!(strip.contains(&"c".into()));
}
