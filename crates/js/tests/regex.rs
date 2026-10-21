//! JavaScript's matching rules: alternatives in order, quantifiers, groups,
//! lookahead, anchors, classes and flags.

use js::regex::Regex;

fn groups(pattern: &str, flags: &str, text: &str) -> Option<Vec<Option<String>>> {
    let caps = Regex::new(pattern, flags).exec(text)?;
    Some(
        (0..caps.len())
            .map(|i| caps.get(i).map(str::to_string))
            .collect(),
    )
}

fn found(pattern: &str, flags: &str, text: &str) -> Vec<Option<&'static str>> {
    groups(pattern, flags, text)
        .expect("a match")
        .into_iter()
        .map(|g| g.map(|g| &*g.leak()))
        .collect()
}

#[test]
fn alternatives_are_tried_in_order() {
    assert_eq!(found("a|ab", "", "abc"), [Some("a")]);
    assert_eq!(
        found("(a|ab)(c|bcd)", "", "abcd"),
        [Some("abcd"), Some("a"), Some("bcd")]
    );
}

#[test]
fn greedy_and_lazy_quantifiers() {
    assert_eq!(found("a+", "", "caaat"), [Some("aaa")]);
    assert_eq!(found("a+?", "", "caaat"), [Some("a")]);
    assert_eq!(found("<.*>", "", "<a><b>"), [Some("<a><b>")]);
    assert_eq!(found("<.*?>", "", "<a><b>"), [Some("<a>")]);
    assert_eq!(found("a{2,3}", "", "aaaa"), [Some("aaa")]);
    assert_eq!(found("a{2,3}?", "", "aaaa"), [Some("aa")]);
    assert_eq!(found("x{2}", "", "xxx"), [Some("xx")]);
    assert_eq!(found("(?:ab){2,}", "", "abababa"), [Some("ababab")]);
    assert_eq!(found("(?:ab)+?b", "", "ababb"), [Some("ababb")]);
    assert_eq!(found("a{,2}", "", "a{,2}"), [Some("a{,2}")]);
}

#[test]
fn a_group_keeps_its_last_iteration() {
    assert_eq!(found("(a|b)+", "", "abab"), [Some("abab"), Some("b")]);
    assert_eq!(found("(?:(a)|b)+", "", "ab"), [Some("ab"), Some("a")]);
    assert_eq!(
        found("(\\w+)\\s(\\w+)", "", "hello big world"),
        [Some("hello big"), Some("hello"), Some("big")]
    );
}

#[test]
fn a_group_that_backtracks_out_takes_no_part() {
    assert_eq!(found("(a)?b", "", "b"), [Some("b"), None]);
    assert_eq!(found("(?:(x)y|xz)", "", "xz"), [Some("xz"), None]);
}

#[test]
fn an_empty_iteration_ends_a_loop() {
    assert_eq!(found("(a*)*", "", "b"), [Some(""), None]);
    assert_eq!(found("(a*)+", "", "b"), [Some(""), Some("")]);
    assert_eq!(found("(?:a?)*?b", "", "aab"), [Some("aab")]);
    assert_eq!(found("(a|)*b", "", "aab"), [Some("aab"), Some("a")]);
}

#[test]
fn lookahead_is_atomic() {
    assert_eq!(found("a(?=b)", "", "acab"), [Some("a")]);
    assert_eq!(Regex::new("a(?=b)", "").exec("acab").unwrap().start(), 2);
    assert_eq!(found("a(?!b)", "", "abac"), [Some("a")]);
    assert_eq!(Regex::new("a(?!b)", "").exec("abac").unwrap().start(), 2);
    assert_eq!(
        found("(?=(a+))a*b", "", "aaab"),
        [Some("aaab"), Some("aaa")]
    );
    assert_eq!(found("(?!(a))b", "", "b"), [Some("b"), None]);
}

#[test]
fn anchors_and_boundaries() {
    assert!(Regex::new("^b", "").exec("a\nb").is_none());
    assert_eq!(found("^b$", "m", "a\nb\nc"), [Some("b")]);
    assert_eq!(found("\\bis\\b", "", "this is"), [Some("is")]);
    assert_eq!(Regex::new("\\Bis", "").exec("this is").unwrap().start(), 2);
    assert_eq!(found(".+", "", "a\nb"), [Some("a")]);
    assert_eq!(found(".+", "s", "a\nb"), [Some("a\nb")]);
}

#[test]
fn classes_and_case() {
    assert_eq!(found("[^a-c]+", "", "abcdefa"), [Some("def")]);
    assert_eq!(found("[a-c]+", "i", "xABCx"), [Some("ABC")]);
    assert_eq!(found("\\d+\\s*\\w", "", "x 12  z"), [Some("12  z")]);
    assert_eq!(found("\\p{L}+", "u", "1 héllo"), [Some("héllo")]);
    assert_eq!(found("ß|é", "i", "É"), [Some("É")]);
}

#[test]
fn sticky_matches_only_at_the_start() {
    let re = Regex::new("b", "y");
    assert!(re.exec_at("ab", 0).is_none());
    assert_eq!(re.exec_at("ab", 1).unwrap().get(0), Some("b"));
}

#[test]
fn split_replace_and_find_all() {
    let re = Regex::new("(,)\\s*", "");
    assert_eq!(
        re.split("a, b,c"),
        [Some("a"), Some(","), Some("b"), Some(","), Some("c")]
    );
    let re = Regex::new("o", "g");
    assert_eq!(re.replace("foo boo", |_| "0".into()), "f00 b00");
    assert_eq!(re.find_all("foo"), ["o", "o"]);
    let re = Regex::new("x*", "g");
    assert_eq!(re.replace("abc", |_| "-".into()), "-a-b-c-");
}

#[test]
fn a_long_input_does_not_exhaust_the_stack() {
    let text = "ab".repeat(100_000);
    assert_eq!(
        Regex::new("(?:a|b)*$", "").exec(&text).unwrap().end(),
        text.len()
    );
    assert_eq!(
        Regex::new("(?:ab)+c|(?:ab)+", "")
            .exec(&text)
            .unwrap()
            .end(),
        text.len()
    );
}

#[test]
fn try_new_reports_what_new_regexp_throws() {
    let error =
        |pattern: &str, flags: &str| Regex::try_new(pattern, flags).unwrap_err().to_string();
    assert_eq!(
        error("a", "q"),
        "Invalid flags supplied to RegExp constructor 'q'"
    );
    assert_eq!(
        error("a", "gg"),
        "Invalid flags supplied to RegExp constructor 'gg'"
    );
    assert_eq!(
        error("(", ""),
        "Invalid regular expression: /(/: Unterminated group"
    );
    assert!(Regex::try_new("a)", "").is_err());
    assert!(Regex::try_new("[a", "").is_err());
    assert!(Regex::try_new("(?<=a)b", "").is_err());
    assert!(Regex::try_new("(?<x>a)(?<x>b)", "").is_err());
    assert!(Regex::try_new("a", "dgimsuy").is_ok());
}

#[test]
fn named_groups_capture_and_substitute() {
    assert_eq!(
        found("(?<x>a)(b)", "", "ab"),
        [Some("ab"), Some("a"), Some("b")]
    );
    let re = Regex::new("(?<first>\\w+)\\s(\\w+)", "");
    assert_eq!(re.replace_str("John Smith", "$2, $<first>"), "Smith, John");
    assert_eq!(re.replace_str("John Smith", "[$<none>]"), "[]");
}

#[test]
fn replace_str_reads_dollar_patterns() {
    assert_eq!(Regex::new("-", "g").replace_str("a-b-c", "+"), "a+b+c");
    assert_eq!(Regex::new("-", "").replace_str("a-b-c", "+"), "a+b-c");
    assert_eq!(
        Regex::new("(b)", "").replace_str("abc", "$2$1$0"),
        "a$2b$0c"
    );
    assert_eq!(Regex::new("(b)", "").replace_str("abc", "$10"), "ab0c");
    assert_eq!(Regex::new("a", "g").replace_str("aaa", "$10"), "$10$10$10");
    assert_eq!(Regex::new("b", "").replace_str("abc", "$<x>"), "a$<x>c");
}
