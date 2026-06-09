use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionMode {
    #[default]
    Manual,
    Classifier,
    Agent,
}

impl DecisionMode {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Manual => 0,
            Self::Classifier => 1,
            Self::Agent => 2,
        }
    }

    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Classifier,
            2 => Self::Agent,
            _ => Self::Manual,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseDecisionModeError(pub String);

impl std::fmt::Display for ParseDecisionModeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid decision mode '{}', expected 'manual', 'classifier', or 'agent'",
            self.0
        )
    }
}

impl std::error::Error for ParseDecisionModeError {}

impl std::str::FromStr for DecisionMode {
    type Err = ParseDecisionModeError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "manual" => Ok(Self::Manual),
            "classifier" => Ok(Self::Classifier),
            "agent" => Ok(Self::Agent),
            other => Err(ParseDecisionModeError(other.to_string())),
        }
    }
}

impl std::fmt::Display for DecisionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Manual => "manual",
            Self::Classifier => "classifier",
            Self::Agent => "agent",
        })
    }
}
