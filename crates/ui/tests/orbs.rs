use bezel_ui::orbs::Orb;

#[test]
fn builder_paused_false_clears_pause_clock() {
    let orb = Orb::new().paused(true).paused(false);
    assert!(!orb.is_paused());
}

#[test]
fn builder_paused_true_records_paused_at() {
    let orb = Orb::new().paused(true);
    assert!(orb.is_paused());
}
