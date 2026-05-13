use loopal_protocol::{Question, UserQuestionResponse};

pub(super) fn format_response(
    response: &UserQuestionResponse,
    questions: &[Question],
) -> (String, bool) {
    match response {
        UserQuestionResponse::Cancelled { .. } => ("(cancelled by user)".to_string(), false),
        UserQuestionResponse::Unsupported { reason, .. } => {
            (format!("(unsupported: {reason})"), true)
        }
        UserQuestionResponse::Answered { answers, .. } => format_answers(answers, questions),
    }
}

#[doc(hidden)]
pub fn format_response_for_test(
    response: &UserQuestionResponse,
    questions: &[Question],
) -> (String, bool) {
    format_response(response, questions)
}

fn format_answers(answers: &[String], questions: &[Question]) -> (String, bool) {
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
