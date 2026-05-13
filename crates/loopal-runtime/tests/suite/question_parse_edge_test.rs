use loopal_runtime::agent_loop::question_parse::parse_questions_for_test as parse_questions;
use serde_json::json;

#[test]
fn missing_questions_field_returns_err() {
    let input = json!({});
    let err = parse_questions(&input).unwrap_err();
    assert!(err.contains("questions"));
    assert!(err.contains("required"));
}

#[test]
fn empty_questions_array_returns_err() {
    let input = json!({"questions": []});
    let err = parse_questions(&input).unwrap_err();
    assert!(err.contains("at least one"));
}

#[test]
fn missing_question_field_returns_err_with_path() {
    let input = json!({
        "questions": [{
            "options": [
                {"label": "A", "description": "a"},
                {"label": "B", "description": "b"}
            ]
        }]
    });
    let err = parse_questions(&input).unwrap_err();
    assert!(err.contains("questions[0].question"));
    assert!(err.contains("non-empty"));
}

#[test]
fn empty_question_string_returns_err() {
    let input = json!({
        "questions": [{
            "question": "",
            "options": [
                {"label": "A", "description": "a"},
                {"label": "B", "description": "b"}
            ]
        }]
    });
    let err = parse_questions(&input).unwrap_err();
    assert!(err.contains("questions[0].question"));
}

#[test]
fn missing_options_field_returns_err() {
    let input = json!({"questions": [{"question": "Q"}]});
    let err = parse_questions(&input).unwrap_err();
    assert!(err.contains("questions[0].options"));
    assert!(err.contains("required"));
}

#[test]
fn options_can_be_empty_for_free_text_only() {
    let input = json!({
        "questions": [{
            "question": "Type freely",
            "options": []
        }]
    });
    let qs = parse_questions(&input).expect("empty options should be allowed");
    assert_eq!(qs[0].options.len(), 0);
}

#[test]
fn options_can_be_single_item() {
    let input = json!({
        "questions": [{
            "question": "Q",
            "options": [{"label": "A", "description": "a"}]
        }]
    });
    let qs = parse_questions(&input).expect("single option allowed");
    assert_eq!(qs[0].options.len(), 1);
}

#[test]
fn option_missing_label_returns_err() {
    let input = json!({
        "questions": [{
            "question": "Q",
            "options": [
                {"description": "missing label"},
                {"label": "B", "description": "b"}
            ]
        }]
    });
    let err = parse_questions(&input).unwrap_err();
    assert!(err.contains("questions[0].options[0].label"));
}

#[test]
fn option_missing_description_returns_err() {
    let input = json!({
        "questions": [{
            "question": "Q",
            "options": [
                {"label": "A", "description": "a"},
                {"label": "B"}
            ]
        }]
    });
    let err = parse_questions(&input).unwrap_err();
    assert!(err.contains("questions[0].options[1].description"));
}

#[test]
fn empty_option_label_returns_err() {
    let input = json!({
        "questions": [{
            "question": "Q",
            "options": [
                {"label": "", "description": "a"},
                {"label": "B", "description": "b"}
            ]
        }]
    });
    let err = parse_questions(&input).unwrap_err();
    assert!(err.contains("questions[0].options[0].label"));
}

#[test]
fn second_question_error_carries_correct_index() {
    let input = json!({
        "questions": [
            {
                "question": "Q1",
                "options": [
                    {"label": "A", "description": "a"},
                    {"label": "B", "description": "b"}
                ]
            },
            {
                "options": [
                    {"label": "X", "description": "x"},
                    {"label": "Y", "description": "y"}
                ]
            }
        ]
    });
    let err = parse_questions(&input).unwrap_err();
    assert!(err.contains("questions[1].question"));
}
