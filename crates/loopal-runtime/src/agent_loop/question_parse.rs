use loopal_protocol::{Question, QuestionOption};

pub(super) fn parse_questions(input: &serde_json::Value) -> Result<Vec<Question>, String> {
    let arr = input
        .get("questions")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            "AskUser parameter validation failed: 'questions' is required and must be an array"
                .to_string()
        })?;

    if arr.is_empty() {
        return Err(
            "AskUser parameter validation failed: 'questions' must contain at least one item"
                .to_string(),
        );
    }

    arr.iter()
        .enumerate()
        .map(|(i, q)| parse_one_question(i, q))
        .collect()
}

fn parse_one_question(i: usize, q: &serde_json::Value) -> Result<Question, String> {
    let question = q
        .get("question")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            format!(
                "AskUser parameter validation failed: questions[{i}].question is required and must be a non-empty string"
            )
        })?
        .to_string();

    let options_raw = q.get("options").and_then(|v| v.as_array()).ok_or_else(|| {
        format!(
            "AskUser parameter validation failed: questions[{i}].options is required and must be an array"
        )
    })?;

    let options: Vec<QuestionOption> = options_raw
        .iter()
        .enumerate()
        .map(|(j, o)| parse_option(i, j, o))
        .collect::<Result<_, _>>()?;

    let allow_multiple = q
        .get("multiSelect")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let header = q
        .get("header")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    Ok(Question {
        question,
        options,
        allow_multiple,
        header,
    })
}

fn parse_option(i: usize, j: usize, o: &serde_json::Value) -> Result<QuestionOption, String> {
    let label = o
        .get("label")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            format!(
                "AskUser parameter validation failed: questions[{i}].options[{j}].label is required and must be a non-empty string"
            )
        })?
        .to_string();

    let description = o
        .get("description")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            format!(
                "AskUser parameter validation failed: questions[{i}].options[{j}].description is required and must be a string"
            )
        })?
        .to_string();

    Ok(QuestionOption { label, description })
}

#[doc(hidden)]
pub fn parse_questions_for_test(input: &serde_json::Value) -> Result<Vec<Question>, String> {
    parse_questions(input)
}
