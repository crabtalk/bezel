use gallery::highlight;

#[test]
fn native_and_generated_samples_have_highlights() {
    mod generated {
        include!(concat!(env!("OUT_DIR"), "/highlights.rs"));
    }

    assert_eq!(highlight::languages(), generated::LANGUAGES);
    for (language, code, expected) in generated::HIGHLIGHTS {
        assert!(!expected.is_empty(), "unhighlighted {language} sample");
        assert_eq!(highlight::spans(language, code).as_deref(), Some(*expected));
    }
}
