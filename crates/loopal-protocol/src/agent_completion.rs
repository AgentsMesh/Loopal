use serde::{Deserialize, Serialize};

pub const NO_AGENT_OUTPUT: &str = "(no output)";
pub const TRANSPORT_ERROR_REASON: &str = "transport_error";
pub const PROTOCOL_ERROR_REASON: &str = "protocol_error";
/// Request-level capability for callers that understand typed wait failures.
/// Older Hubs ignore the extra request field; newer Hubs use it to preserve raw
/// partial output for typed consumers while protecting legacy consumers.
pub const WAIT_AGENT_TYPED_RESPONSE_V1: &str = "typed_v1";

/// Authoritative terminal result carried by the `agent/completed` IPC notification.
///
/// Stream events are observational UI output. Callers must use `result` as the
/// single source of truth for the value returned by a completed agent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCompletion {
    pub reason: String,
    #[serde(default)]
    pub result: Option<String>,
}

impl AgentCompletion {
    pub fn new(reason: impl Into<String>, result: Option<String>) -> Self {
        Self {
            reason: reason.into(),
            result,
        }
    }

    pub fn goal(result: Option<String>) -> Self {
        Self::new("goal", result)
    }

    /// Whether the terminal reason represents successful task completion.
    ///
    /// Only `goal` is success. All other and unknown reasons fail closed so a
    /// new failure reason cannot become a successful Agent tool result by accident.
    pub fn is_success(&self) -> bool {
        self.reason == "goal"
    }

    /// Stable text projection for legacy consumers and parent-message delivery.
    /// The typed `result` remains authoritative and continues to distinguish
    /// `None` from an explicitly empty result.
    pub fn output(&self) -> &str {
        self.result.as_deref().unwrap_or(NO_AGENT_OUTPUT)
    }

    /// Best available diagnostic for a non-successful completion.
    pub fn failure_detail(&self) -> &str {
        self.result
            .as_deref()
            .filter(|result| !result.is_empty())
            .unwrap_or(&self.reason)
    }
}

/// Terminal state returned by the `hub/wait_agent` RPC.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaitAgentStatus {
    Completed,
    Failed,
    TimedOut,
    NotFound,
}

/// Typed `hub/wait_agent` response.
///
/// `output` is intentionally present for every status. Existing successful
/// callers can keep reading it, while `status` prevents failures, timeouts, and
/// lookup misses from masquerading as successful text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaitAgentResponse {
    pub status: WaitAgentStatus,
    pub reason: String,
    pub output: String,
    /// Legacy timeout discriminator retained during the typed-wire migration.
    #[serde(default, skip_serializing_if = "is_false")]
    pub timed_out: bool,
}

impl WaitAgentResponse {
    pub fn from_completion(completion: AgentCompletion) -> Self {
        let status = if completion.is_success() {
            WaitAgentStatus::Completed
        } else {
            WaitAgentStatus::Failed
        };
        Self {
            status,
            output: completion.output().to_string(),
            reason: completion.reason,
            timed_out: false,
        }
    }

    pub fn timed_out() -> Self {
        Self {
            status: WaitAgentStatus::TimedOut,
            reason: "timeout".into(),
            output: "(agent timed out)".into(),
            timed_out: true,
        }
    }

    pub fn not_found() -> Self {
        Self {
            status: WaitAgentStatus::NotFound,
            reason: "not_found".into(),
            output: "agent not found or already finished".into(),
            timed_out: false,
        }
    }

    /// Text-safe projection for old consumers that read only `output` and
    /// therefore cannot observe the typed terminal status.
    pub fn legacy_safe_output(&self) -> String {
        if self.status == WaitAgentStatus::Completed {
            return self.output.clone();
        }
        let marker = format!("[agent completion failed; reason: {}]", self.reason);
        if self.output.is_empty() {
            marker
        } else {
            format!("{marker}\n{}", self.output)
        }
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}
