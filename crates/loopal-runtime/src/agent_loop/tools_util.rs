//! Helpers for AskUser tool interception: parse questions JSON and format answers.

use loopal_protocol::{Question, QuestionOption};

pub(super) fn parse_questions(input: &serde_json::Value) -> Vec<Question> {
    let Some(questions) = input.get("questions").and_then(|v| v.as_array()) else {
        return vec![Question {
            question: "?".into(),
            options: Vec::new(),
            allow_multiple: false,
        }];
    };
    questions
        .iter()
        .map(|q| {
            let question = q
                .get("question")
                .and_then(|v| v.as_str())
                .unwrap_or("?")
                .to_string();
            let allow_multiple = q
                .get("multiSelect")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let options = q
                .get("options")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(|o| QuestionOption {
                            label: o
                                .get("label")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            description: o
                                .get("description")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            Question {
                question,
                options,
                allow_multiple,
            }
        })
        .collect()
}

pub(super) fn format_answers(answers: &[String]) -> String {
    if answers.is_empty() {
        return "(no selection)".to_string();
    }
    answers.join(", ")
}
