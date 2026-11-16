use crate::page::Report;

#[test]
fn async_replies_keep_the_request_id_and_javascript_error() {
    let reply = Report::evaluated(r#"{"evaluation":12,"value":"{\"ok\":true}"}"#).unwrap();
    assert!(
        matches!(reply, Report::Evaluated { id: 12, answer: Ok(value) } if value == r#"{"ok":true}"#)
    );
    let reply =
        Report::evaluated(r#"{"evaluation":13,"error":"Error: failed\n at submit"}"#).unwrap();
    assert!(
        matches!(reply, Report::Evaluated { id: 13, answer: Err(error) } if error == "Error: failed\n at submit")
    );
}

#[test]
fn console_and_malformed_messages_are_not_evaluation_replies() {
    for body in [
        r#"{"level":"error","text":"failed"}"#,
        r#"{"evaluation":1}"#,
        r#"{"evaluation":1,"value":"null","error":"failed"}"#,
        r#"{"evaluation":1,"value":42}"#,
    ] {
        assert!(Report::evaluated(body).is_none());
    }
}
