use gallery::patterns::document::SOURCE;

/// The Source segment shows `serialize(&doc)` and the page invites you to
/// compare it with [`SOURCE`]. If the constant drifts out of canonical form
/// the two stop matching, and the screen quietly demonstrates a round trip
/// it does not actually survive.
#[test]
fn the_source_is_canonical() {
    let doc = markdown::parse(SOURCE);
    assert_eq!(markdown::serialize(&doc), SOURCE);
}
