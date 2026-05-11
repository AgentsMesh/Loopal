use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionLevel {
    ReadOnly,
    Write,
    Dangerous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    Bypass,
    AskDangerous,
    AskAnyWrite,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsePermissionModeError(pub String);

impl std::fmt::Display for ParsePermissionModeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid permission mode '{}', expected one of: bypass, ask_dangerous, ask_any_write",
            self.0
        )
    }
}

impl std::error::Error for ParsePermissionModeError {}

impl std::str::FromStr for PermissionMode {
    type Err = ParsePermissionModeError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "bypass" => Ok(Self::Bypass),
            "ask_dangerous" => Ok(Self::AskDangerous),
            "ask_any_write" => Ok(Self::AskAnyWrite),
            other => Err(ParsePermissionModeError(other.to_string())),
        }
    }
}

impl std::fmt::Display for PermissionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Bypass => "bypass",
            Self::AskDangerous => "ask_dangerous",
            Self::AskAnyWrite => "ask_any_write",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionDecision {
    Allow,
    Ask,
    Deny,
}

impl PermissionDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Ask => "ask",
            Self::Deny => "deny",
        }
    }
}

impl PermissionMode {
    pub fn check(self, level: PermissionLevel) -> PermissionDecision {
        match (self, level) {
            (Self::Bypass, _) => PermissionDecision::Allow,
            (Self::AskDangerous, PermissionLevel::Dangerous) => PermissionDecision::Ask,
            (Self::AskDangerous, _) => PermissionDecision::Allow,
            (Self::AskAnyWrite, PermissionLevel::ReadOnly) => PermissionDecision::Allow,
            (Self::AskAnyWrite, _) => PermissionDecision::Ask,
        }
    }
}
