use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ResolveSource {
    #[default]
    Manual,
    Classifier,
    Agent,
}

impl ResolveSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Classifier => "classifier",
            Self::Agent => "agent",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub question: String,
    pub options: Vec<QuestionOption>,
    pub allow_multiple: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionOption {
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UserQuestionResponse {
    Answered {
        question_id: String,
        answers: Vec<String>,
    },
    Cancelled {
        question_id: String,
    },
    Unsupported {
        question_id: String,
        reason: String,
    },
}

impl UserQuestionResponse {
    pub fn answered(question_id: impl Into<String>, answers: Vec<String>) -> Self {
        Self::Answered {
            question_id: question_id.into(),
            answers,
        }
    }

    pub fn cancelled(question_id: impl Into<String>) -> Self {
        Self::Cancelled {
            question_id: question_id.into(),
        }
    }

    pub fn unsupported(question_id: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Unsupported {
            question_id: question_id.into(),
            reason: reason.into(),
        }
    }

    pub fn question_id(&self) -> &str {
        match self {
            Self::Answered { question_id, .. }
            | Self::Cancelled { question_id }
            | Self::Unsupported { question_id, .. } => question_id,
        }
    }
}
