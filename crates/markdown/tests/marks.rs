//! The marks an app spells itself: read, written, and above all round tripped.
//!
//! The fixed point is the whole reason the registry is threaded through the
//! parse rather than run over the text afterwards — a pass over a parsed
//! [`Text`] cannot tell `==` from `\=\=`, because CommonMark has already taken
//! the backslashes off.

use markdown::{BlockKind, Mark, Marks, Part, Text, parse_with, serialize_with};

fn marks() -> Marks {
    Marks::new().with("highlight", "==").with("underline", "++")
}

fn body(doc: &markdown::Doc, ix: usize) -> &Text {
    doc.blocks[ix].text_at(Part::Body).expect("a body")
}

#[test]
fn a_registered_delimiter_becomes_a_mark() {
    let doc = parse_with("a ==lit== word", &marks());
    let text = body(&doc, 0);
    assert_eq!(text.text, "a lit word");
    assert_eq!(
        text.marks
            .iter()
            .map(|span| (span.range.clone(), span.mark.clone()))
            .collect::<Vec<_>>(),
        vec![(2..5, Mark::Custom("highlight".into()))]
    );
}

#[test]
fn an_escaped_delimiter_stays_text() {
    let doc = parse_with(r"a \=\=lit\=\= word", &marks());
    let text = body(&doc, 0);
    assert_eq!(text.text, "a ==lit== word", "the escapes are gone");
    assert!(text.marks.is_empty(), "and nothing was marked");
}

#[test]
fn a_delimiter_in_code_is_code() {
    let doc = parse_with("`a ==lit== b`\n\n```\n==fenced==\n```", &marks());
    assert!(
        body(&doc, 0)
            .marks
            .iter()
            .all(|span| span.mark == Mark::Code)
    );
    let BlockKind::Code { code, .. } = &doc.blocks[1].kind else {
        panic!("expected a fence")
    };
    assert_eq!(code.text, "==fenced==", "a fence is literal to its close");
}

#[test]
fn a_delimiter_in_a_destination_is_a_destination() {
    let doc = parse_with("[a](https://x.test/==b==)", &marks());
    let text = body(&doc, 0);
    assert_eq!(
        text.marks.first().map(|span| span.mark.clone()),
        Some(Mark::Link("https://x.test/==b==".into())),
        "the URL keeps every character it was written with"
    );
}

#[test]
fn marks_nest_with_the_ones_markdown_already_spells() {
    let marks = marks();
    for source in [
        "**==both==**",
        "==**both**==",
        "a ==lit **and bold** here== b",
        "++under== crossed ==++",
    ] {
        let doc = parse_with(source, &marks);
        let written = serialize_with(&doc, &marks);
        assert_eq!(
            parse_with(&written, &marks),
            doc,
            "{source:?} wrote {written:?}"
        );
    }
}

#[test]
fn the_fixed_point_holds_with_marks_registered() {
    let marks = marks();
    for source in [
        "a ==lit== word",
        r"a \=\=literal\=\= word",
        "== not a mark, it opened on a space",
        "a ==lit==",
        "==lit== at the start",
        "# A ==heading==\n\n- a ++bullet++\n\n> a ==quote==",
        "| a ==cell== | b |\n| --- | --- |\n| c | ++d++ |",
        "![a ==caption==](https://x.test/i.png)",
        "a == b == c",
        "==",
        "++==both kinds==++",
    ] {
        let doc = parse_with(source, &marks);
        let written = serialize_with(&doc, &marks);
        let again = parse_with(&written, &marks);
        assert_eq!(again, doc, "{source:?} wrote {written:?}");
        assert_eq!(
            serialize_with(&again, &marks),
            written,
            "and writes the same thing twice"
        );
    }
}

#[test]
fn an_unregistered_document_reads_the_delimiters_as_text() {
    let doc = parse_with("a ==lit== word", &Marks::default());
    assert_eq!(body(&doc, 0).text, "a ==lit== word");
    assert!(body(&doc, 0).marks.is_empty());
}

#[test]
fn a_name_with_no_delimiter_writes_nothing_and_loses_nothing() {
    // A document carrying a mark this registry does not spell — an app that
    // dropped a registration, or a `Doc` built by hand.
    let doc = parse_with("a ==lit== word", &marks());
    let written = serialize_with(&doc, &Marks::default());
    assert_eq!(
        written, "a lit word",
        "the text survives, the mark does not"
    );
}

#[test]
fn a_registration_cannot_be_taken_twice() {
    let marks = Marks::new()
        .with("highlight", "==")
        .with("other", "==")
        .with("highlight", "!!");
    assert_eq!(marks.names().count(), 1);
    assert_eq!(marks.delimiter("highlight"), Some("=="));
}

/// The same generated sweep [`serialize`]'s own fixed-point test runs, with a
/// registry on: the delimiters are in the fragments, so the marks land in the
/// middle of headings, tables, fences and each other.
#[test]
fn the_fixed_point_holds_over_generated_documents() {
    const FRAGMENTS: &[&str] = &[
        "a ==lit== word",
        r"a \=\=literal\=\= word",
        "==",
        "++",
        "== a",
        "a ==",
        "# ==heading==",
        "- ==item== and ++more++",
        "> ==quoted",
        "`==code==`",
        "```\n==fenced==\n```",
        "| ==a== | b |",
        "| --- | --- |",
        "**==both==**",
        "==**both**==",
        "++==nested==++",
        "[a](https://x.test/==b==)",
        "![alt ==x==](https://x.test/i.png)",
        "plain",
        "",
    ];
    let marks = marks();
    let mut seed = 0x5eedu64;
    let mut next = move || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 33) as usize
    };

    for case in 0..20_000 {
        let lines = 1 + next() % 8;
        let source = (0..lines)
            .map(|_| FRAGMENTS[next() % FRAGMENTS.len()])
            .collect::<Vec<_>>()
            .join("\n");

        let once = parse_with(&source, &marks);
        let written = serialize_with(&once, &marks);
        let twice = parse_with(&written, &marks);
        assert_eq!(
            once, twice,
            "case {case} is not a fixed point\n--- source ---\n{source}\n--- written ---\n{written}\n"
        );
        assert_eq!(
            serialize_with(&twice, &marks),
            written,
            "case {case} drifted on a second pass\n--- source ---\n{source}\n"
        );
    }
}
