//! Sessions hold their own compiled queries.

mod fixture;

use syntax::session::{self, Session};

const PAGE: &str = "<style>.a { color: red; }</style>";

/// Two sessions paint the same source the same way. Neither shares a config
/// with the other, which is what lets each own the store its grammars came
/// from.
#[test]
fn separate_sessions_agree() {
    fixture::install();
    let lang = syntax::registry::of_tag("html")
        .and_then(|known| known.lang())
        .expect("html was registered");

    let first = Session::new().highlight(lang, PAGE);
    let second = Session::new().highlight(lang, PAGE);

    assert!(first.as_ref().is_some_and(|spans| !spans.is_empty()));
    assert_eq!(first, second);
}

/// The thread-local session answers the same as one built by hand.
#[test]
fn the_thread_local_session_agrees_with_a_fresh_one() {
    fixture::install();
    let lang = syntax::registry::of_tag("html")
        .and_then(|known| known.lang())
        .expect("html was registered");

    let owned = Session::new().highlight(lang, PAGE);
    let shared = session::with(|session| session.highlight(lang, PAGE));

    assert_eq!(owned, shared);
}

/// A session compiles what an injection names without being asked for it: the
/// `<style>` body paints on the first parse, with nothing having reached for
/// css beforehand.
#[test]
fn an_injected_language_is_compiled_before_the_parse() {
    fixture::install();
    let lang = syntax::registry::of_tag("html")
        .and_then(|known| known.lang())
        .expect("html was registered");

    let spans = Session::new().highlight(lang, PAGE).expect("html paints");
    let at = PAGE.find("color").expect("the property is in the source");
    assert!(
        spans
            .iter()
            .any(|(range, _)| range.start <= at && at < range.end),
        "the css body did not paint, so its config was never compiled"
    );
}

/// Each thread gets its own session, and a parse on one does not disturb
/// another.
#[test]
fn sessions_are_per_thread() {
    fixture::install();
    let lang = syntax::registry::of_tag("html")
        .and_then(|known| known.lang())
        .expect("html was registered");

    let here = session::with(|session| session.highlight(lang, PAGE));
    let there = std::thread::spawn(move || session::with(|session| session.highlight(lang, PAGE)))
        .join()
        .expect("the thread finished");

    assert_eq!(here, there);
}
