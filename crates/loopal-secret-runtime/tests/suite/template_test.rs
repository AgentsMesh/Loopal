use loopal_secret_runtime::{
    TranslationView, collect_author_names, collect_wire_names, translate_outbound,
};

fn known(names: &[&str]) -> TranslationView {
    TranslationView::from_names(names.iter().map(|s| s.to_string()))
}

#[test]
fn translates_known_secret_to_wire_form() {
    let v = known(&["openai_key"]);
    let (out, stats) = translate_outbound("token={{secret:openai_key}}", Some(&v));
    assert_eq!(out, "token=<secret_ref:openai_key>");
    assert_eq!(stats.translated, 1);
    assert!(stats.missing.is_empty());
}

#[test]
fn unknown_becomes_missing_placeholder() {
    let v = known(&["other"]);
    let (out, stats) = translate_outbound("k={{secret:ghost}}", Some(&v));
    assert_eq!(out, "k=<missing-secret:ghost>");
    assert_eq!(stats.translated, 0);
    assert_eq!(stats.missing, vec!["ghost".to_string()]);
}

#[test]
fn uppercase_name_rejected_by_strict_regex() {
    let v = known(&["FOO"]);
    let (out, stats) = translate_outbound("x={{secret:FOO}}", Some(&v));
    assert_eq!(out, "x={{secret:FOO}}");
    assert_eq!(stats.translated, 0);
}

#[test]
fn leading_digit_name_rejected() {
    let v = known(&["1foo"]);
    let (out, _) = translate_outbound("x={{secret:1foo}}", Some(&v));
    assert_eq!(out, "x={{secret:1foo}}");
}

#[test]
fn empty_name_rejected() {
    let v = known(&[]);
    let (out, _) = translate_outbound("x={{secret:}}", Some(&v));
    assert_eq!(out, "x={{secret:}}");
}

#[test]
fn multiple_placeholders_in_same_string() {
    let v = known(&["a", "b"]);
    let (out, stats) = translate_outbound("A={{secret:a}}, B={{secret:b}}", Some(&v));
    assert_eq!(out, "A=<secret_ref:a>, B=<secret_ref:b>");
    assert_eq!(stats.translated, 2);
}

#[test]
fn translation_view_none_marks_everything_missing() {
    let (out, stats) = translate_outbound("k={{secret:foo}}", None);
    assert_eq!(out, "k=<missing-secret:foo>");
    assert_eq!(stats.missing, vec!["foo".to_string()]);
}

#[test]
fn collect_author_names_finds_all() {
    let names = collect_author_names("{{secret:a}} and {{secret:b_c}}");
    assert_eq!(names, vec!["a".to_string(), "b_c".to_string()]);
}

#[test]
fn collect_wire_names_finds_all() {
    let names = collect_wire_names("<secret_ref:a> and <secret_ref:b_c>");
    assert_eq!(names, vec!["a".to_string(), "b_c".to_string()]);
}

#[test]
fn unrelated_braces_not_touched() {
    let v = known(&["foo"]);
    let (out, _) = translate_outbound("vars { foo bar } end", Some(&v));
    assert_eq!(out, "vars { foo bar } end");
}
