use loopal_runtime::agent_loop::question_parse::parse_questions_for_test as parse_questions;
use serde_json::json;

#[test]
fn happy_single_question_with_two_options() {
    let input = json!({
        "questions": [{
            "question": "Pick one",
            "options": [
                {"label": "A", "description": "first"},
                {"label": "B", "description": "second"}
            ]
        }]
    });
    let qs = parse_questions(&input).expect("should parse");
    assert_eq!(qs.len(), 1);
    assert_eq!(qs[0].question, "Pick one");
    assert_eq!(qs[0].options.len(), 2);
    assert!(!qs[0].allow_multiple);
}

#[test]
fn happy_multi_question_with_multi_select() {
    let input = json!({
        "questions": [
            {
                "question": "Q1",
                "multiSelect": true,
                "options": [
                    {"label": "A", "description": "a"},
                    {"label": "B", "description": "b"}
                ]
            },
            {
                "question": "Q2",
                "options": [
                    {"label": "X", "description": "x"},
                    {"label": "Y", "description": "y"},
                    {"label": "Z", "description": "z"}
                ]
            }
        ]
    });
    let qs = parse_questions(&input).expect("should parse");
    assert_eq!(qs.len(), 2);
    assert!(qs[0].allow_multiple);
    assert!(!qs[1].allow_multiple);
    assert_eq!(qs[1].options.len(), 3);
}

#[test]
fn header_field_is_optional_and_ignored() {
    let input = json!({
        "questions": [{
            "header": "Short label",
            "question": "Full text",
            "options": [
                {"label": "A", "description": "a"},
                {"label": "B", "description": "b"}
            ]
        }]
    });
    let qs = parse_questions(&input).expect("should parse");
    assert_eq!(qs[0].question, "Full text");
}

#[test]
fn multi_select_default_false() {
    let input = json!({
        "questions": [{
            "question": "Q",
            "options": [
                {"label": "A", "description": "a"},
                {"label": "B", "description": "b"}
            ]
        }]
    });
    let qs = parse_questions(&input).expect("should parse");
    assert!(!qs[0].allow_multiple);
}
