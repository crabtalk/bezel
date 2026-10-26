use markdown::{Html, html, parse};

fn render(source: &str) -> String {
    html(&parse(source))
}

#[test]
fn prose_and_marks() {
    assert_eq!(
        render("# Set Up\n\nSome **bold _both_** and `x < y`\nnext"),
        "<h1 id=\"set-up\">Set Up</h1><p>Some <strong>bold <em>both</em></strong> and <code>x &lt; y</code><br>next</p>"
    );
}

#[test]
fn repeated_headings_take_suffixes() {
    assert_eq!(
        render("# A\n\n# A"),
        "<h1 id=\"a\">A</h1><h1 id=\"a-1\">A</h1>"
    );
}

#[test]
fn lists_nest_inside_their_items() {
    assert_eq!(
        render("- a\n    - b\n- c\n\n3. d\n4. e\n\n- [x] f"),
        "<ul><li>a<ul><li>b</li></ul></li><li>c</li></ul>\
         <ol start=\"3\"><li>d</li><li>e</li></ol>\
         <ul class=\"md-tasks\"><li class=\"md-task\"><input checked=\"\" disabled=\"\" type=\"checkbox\"> f</li></ul>"
    );
}

#[test]
fn a_paragraph_under_an_item_stays_in_it() {
    assert_eq!(
        render("- a\n\n    more\n- b"),
        "<ul><li>a<p>more</p></li><li>b</li></ul>"
    );
}

#[test]
fn quotes_and_alerts() {
    assert_eq!(
        render("> a\n>\n> b\n\nafter"),
        "<blockquote><p>a</p><p>b</p></blockquote><p>after</p>"
    );
    assert_eq!(
        render("> [!TIP]\n> try it"),
        "<blockquote class=\"md-alert md-alert-tip\"><p class=\"md-alert-title\">Tip</p><p>try it</p></blockquote>"
    );
}

#[test]
fn a_quote_inside_an_item() {
    assert_eq!(
        render("- a\n\n    > q\n- b"),
        "<ul><li>a<blockquote><p>q</p></blockquote></li><li>b</li></ul>"
    );
}

#[test]
fn code_table_rule_and_pictures() {
    assert_eq!(
        render("```rust\nfn a() {}\n```"),
        "<pre><code class=\"language-rust\">fn a() {}\n</code></pre>"
    );
    assert_eq!(
        render("| a | b |\n| --- | :-: |\n| 1 | 2 |"),
        "<table><thead><tr><th>a</th><th style=\"text-align:center\">b</th></tr></thead>\
         <tbody><tr><td>1</td><td style=\"text-align:center\">2</td></tr></tbody></table>"
    );
    assert_eq!(render("---"), "<hr>");
    assert_eq!(
        render("![cap|480](a.png)"),
        "<figure class=\"md-image\"><img src=\"a.png\" alt=\"cap\" width=\"480\"><figcaption>cap</figcaption></figure>"
    );
    assert_eq!(
        render("see ![x](u.png) here"),
        "<p>see <img src=\"u.png\" alt=\"x\"> here</p>"
    );
}

#[test]
fn links_and_bookmarks() {
    assert_eq!(
        render("[a \"b\"](https://x.dev/?a&b)"),
        "<p><a href=\"https://x.dev/?a&amp;b\">a &quot;b&quot;</a></p>"
    );
    assert_eq!(
        render("<https://x.dev>"),
        "<p><a class=\"md-bookmark\" href=\"https://x.dev\">https://x.dev</a></p>"
    );
}

#[test]
fn overlapping_marks_close_and_reopen() {
    let mut doc = parse("abcd");
    let markdown::BlockKind::Paragraph(text) = &mut doc.blocks[0].kind else {
        panic!("a paragraph");
    };
    text.marks = vec![
        markdown::MarkSpan {
            range: 0..3,
            mark: markdown::Mark::Bold,
        },
        markdown::MarkSpan {
            range: 1..4,
            mark: markdown::Mark::Italic,
        },
    ];
    assert_eq!(html(&doc), "<p><strong>a<em>bc</em></strong><em>d</em></p>");
}

#[test]
fn urls_are_encoded_and_unsafe_ones_dropped() {
    assert_eq!(
        render("[a](/f%C3%B6ö\\\\x) [b](javascript:alert(1)) ![c](data:image/png;base64,AA)"),
        "<p><a href=\"/f%C3%B6%C3%B6%5Cx\">a</a> <a href=\"\">b</a> <img src=\"data:image/png;base64,AA\" alt=\"c\"></p>"
    );
}

#[test]
fn hooks_resolve_urls_and_render_fences() {
    let html = Html::new()
        .url(|url| format!("/site/{url}"))
        .fence(|language, code| (language == "mermaid").then(|| format!("<svg>{code}</svg>")))
        .render(&parse(
            "[a](b)\n\n```mermaid 300\ngraph\n```\n\n```sh\nls\n```",
        ));
    assert_eq!(
        html,
        "<p><a href=\"/site/b\">a</a></p>\
         <div class=\"md-fence\" data-language=\"mermaid\" style=\"height:300px\"><svg>graph</svg></div>\
         <pre><code class=\"language-sh\">ls\n</code></pre>"
    );
}

#[test]
fn highlighted_code_takes_classes() {
    let html = Html::new()
        .highlighter(|_, code| {
            Some(vec![(
                0..code.find(' ').unwrap_or(0),
                theme::HighlightKind::Keyword,
            )])
        })
        .render(&parse("```rust\nfn <a>\n```"));
    assert_eq!(
        html,
        "<pre><code class=\"language-rust\"><span class=\"hl-keyword\">fn</span> &lt;a&gt;\n</code></pre>"
    );
}

#[test]
fn the_stylesheet_names_what_the_markup_writes() {
    let css = Html::stylesheet(
        &theme::Theme::light(),
        &theme::Theme::dark(),
        &markdown::Typography::default(),
    );
    for name in [
        ".md h1",
        ".md-alert-tip",
        ".hl-keyword",
        "--md-hl-string-special",
        "prefers-color-scheme: dark",
    ] {
        assert!(css.contains(name), "{name} missing");
    }
}
