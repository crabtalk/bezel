//! A source text edited in place answers what one built from the new text
//! does.

use gpui::{App, TestAppContext};
use markdown::SourceText;

const BEFORE: &str =
    "# Title\n\nSome **bold** text and `code`.\n\n- one\n- two\n\n```rust\nfn main() {}\n```\n";

/// Every edit applied to [`BEFORE`], as the text after it.
fn afters() -> Vec<String> {
    let mut afters = vec![
        // A character inside a line.
        BEFORE.replacen("bold", "bolder", 1),
        // A line split, and two joined.
        BEFORE.replacen("text and", "text\nand", 1),
        BEFORE.replacen("- one\n- two", "- one- two", 1),
        // At either end.
        format!("> {BEFORE}"),
        format!("{BEFORE}tail"),
        BEFORE.trim_end().to_owned(),
        // Across many lines, and everything.
        BEFORE.replacen("**bold** text and `code`.\n\n- one", "x", 1),
        String::new(),
        "a\nb".to_owned(),
        // Opening a fence changes the meaning of everything after it.
        BEFORE.replacen("# Title", "```\n# Title", 1),
        // Non-ASCII on either side of the change.
        BEFORE.replacen("Some", "Sömé", 1),
    ];
    afters.push(BEFORE.to_owned());
    afters
}

fn assert_matches(edited: &SourceText, after: &str, cx: &App) {
    let fresh = SourceText::new(after, cx);
    assert_eq!(edited.text(), after);
    assert_eq!(edited.lines(), fresh.lines(), "lines of {after:?}");
    for offset in 0..=after.len() + 1 {
        assert_eq!(
            edited.line_of(offset),
            fresh.line_of(offset),
            "line of {offset} in {after:?}"
        );
    }
}

#[gpui::test]
fn an_edit_answers_what_a_fresh_build_does(cx: &mut TestAppContext) {
    cx.update(|cx| {
        for after in afters() {
            let mut edited = SourceText::new(BEFORE, cx);
            let job = edited.edit(&after, cx);
            assert_eq!(job.is_some(), after != BEFORE, "a job only for a change");
            assert_matches(&edited, &after, cx);
            if let Some(job) = job {
                assert!(edited.set_spans(job.run()));
            }
            assert_eq!(
                edited.spans(),
                SourceText::new(after.as_str(), cx).spans(),
                "spans of {after:?}"
            );
        }
    });
}

#[gpui::test]
fn edits_in_a_row_stay_in_step(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let mut edited = SourceText::new(BEFORE, cx);
        for after in afters() {
            edited.edit(&after, cx);
            assert_matches(&edited, &after, cx);
        }
    });
}

/// Until the job lands, the spans are the old ones moved past the edit: in
/// order, apart, inside the text, and none over what the edit wrote.
#[gpui::test]
fn spans_before_the_job_skip_the_edit(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let after = BEFORE.replacen("bold", "bolder", 1);
        let mut edited = SourceText::new(BEFORE, cx);
        let _job = edited.edit(&after, cx);
        let spans = edited.spans().expect("markdown is coloured");
        let written = BEFORE.find("bold").unwrap() + "bold".len();
        for pair in spans.windows(2) {
            assert!(pair[0].0.end <= pair[1].0.start, "{pair:?}");
        }
        for (range, _) in spans {
            assert!(
                range.start < range.end && range.end <= after.len(),
                "{range:?}"
            );
            assert!(!(range.start..range.end).contains(&written), "{range:?}");
        }
    });
}

#[gpui::test]
fn a_job_for_an_older_text_is_refused(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let mut edited = SourceText::new(BEFORE, cx);
        let stale = edited.edit("first", cx).unwrap();
        let current = edited.edit("second", cx).unwrap();
        assert!(!edited.set_spans(stale.run()));
        assert!(edited.set_spans(current.run()));
    });
}

#[gpui::test]
fn a_new_highlighter_highlights_the_text_again(cx: &mut TestAppContext) {
    use markdown::AppExt as _;
    use theme::HighlightKind;

    cx.update(|cx| {
        let mut view = SourceText::new(BEFORE, cx);
        assert!(view.rehighlight(cx).is_none(), "nothing has changed");

        cx.set_highlighter(
            |_, code| Some(vec![(0..code.len(), HighlightKind::Keyword)]),
            ["markdown"],
        );
        let job = view
            .rehighlight(cx)
            .expect("a new highlighter rehighlights");
        assert!(view.rehighlight(cx).is_none(), "once");
        assert!(view.set_spans(job.run()));
        assert_eq!(
            view.spans(),
            Some(&[(0..BEFORE.len(), HighlightKind::Keyword)][..])
        );
    });
}
