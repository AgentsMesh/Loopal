use loopal_protocol::{Question, QuestionOption, UserQuestionResponse};

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

pub(super) fn format_response(
    response: &UserQuestionResponse,
    questions: &[Question],
) -> (String, bool) {
    format_response_impl(response, questions)
}

#[doc(hidden)]
pub fn format_response_for_test(
    response: &UserQuestionResponse,
    questions: &[Question],
) -> (String, bool) {
    format_response_impl(response, questions)
}

fn format_response_impl(response: &UserQuestionResponse, questions: &[Question]) -> (String, bool) {
    match response {
        UserQuestionResponse::Cancelled { .. } => ("(cancelled by user)".to_string(), false),
        UserQuestionResponse::Unsupported { reason, .. } => {
            (format!("(unsupported: {reason})"), true)
        }
        UserQuestionResponse::Answered { answers, .. } => {
            if answers.is_empty() {
                return ("(no selection)".to_string(), false);
            }
            if answers.len() != questions.len() {
                tracing::warn!(
                    answers = answers.len(),
                    questions = questions.len(),
                    "AskUser response/questions length mismatch"
                );
                return (
                    format!(
                        "(protocol mismatch: {} answers, {} questions)",
                        answers.len(),
                        questions.len()
                    ),
                    true,
                );
            }
            let text = answers
                .iter()
                .enumerate()
                .map(|(i, ans)| {
                    let q_text = questions.get(i).map(|q| q.question.as_str()).unwrap_or("?");
                    format!("Q{} ({q_text}): {ans}", i + 1)
                })
                .collect::<Vec<_>>()
                .join("\n");
            (text, false)
        }
    }
}
