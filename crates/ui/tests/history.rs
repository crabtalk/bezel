use ui::history::SnapshotHistory;

#[test]
fn coalesced_edits_drop_redo_and_limits_keep_the_nearest_steps() {
    let mut history = SnapshotHistory::new(3);
    history.record(Some(0));
    history.record(Some(1));
    history.record(Some(2));
    assert_eq!(history.undo(|| 3), Some(2));
    assert_eq!(history.undo(|| 2), Some(1));
    history.record(None);
    assert_eq!(
        history.redo(|| 8),
        None,
        "even a coalesced edit starts a new branch"
    );
    history.record(Some(8));
    history.record(Some(9));
    history.set_limit(1);
    assert_eq!(history.undo(|| 10), Some(9));
    assert_eq!(history.undo(|| 9), None);
    assert_eq!(history.redo(|| 9), Some(10));
}

#[test]
fn zero_limit_and_empty_history_never_restore_or_capture() {
    let mut history = SnapshotHistory::new(0);
    history.record(Some(0));
    assert_eq!(
        history.undo(|| panic!("empty history must not capture")),
        None
    );
    assert_eq!(
        history.redo(|| panic!("empty history must not capture")),
        None
    );
}
