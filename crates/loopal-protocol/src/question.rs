use serde::{Deserialize, Serialize};

/// A question to be presented to the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub question: String,
    pub options: Vec<QuestionOption>,
    pub allow_multiple: bool,
}

/// A selectable option within a question.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionOption {
    pub label: String,
    pub description: String,
}

/// User's response to a set of questions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserQuestionResponse {
    /// Selected option labels.
    pub answers: Vec<String>,
}
