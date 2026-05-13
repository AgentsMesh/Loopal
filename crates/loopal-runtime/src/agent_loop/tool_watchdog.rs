use std::time::Duration;

use loopal_tool_api::{TimeoutSecs, ToolResult};
use loopal_tool_invocation::{StaleReason, ToolResultMetadata};
use serde_json::Value;

const GRACE: Duration = Duration::from_secs(30);
const BASH_MAX_TIMEOUT: Duration = Duration::from_secs(600);

pub fn watchdog_deadline(tool_name: &str, input: &Value) -> Option<Duration> {
    if tool_name != "Bash" {
        return None;
    }
    let timeout = TimeoutSecs::from_tool_input(input, 300).to_duration_clamped(BASH_MAX_TIMEOUT);
    Some(timeout + GRACE)
}

pub fn timeout_result(deadline: Duration) -> ToolResult {
    let secs = deadline.as_secs();
    ToolResult {
        content: format!(
            "Watchdog timeout: tool did not return within {secs}s (timeout + 30s grace)",
        ),
        is_error: true,
        metadata: Some(ToolResultMetadata::stale(StaleReason::WatchdogTimeout)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn watchdog_only_for_bash() {
        assert!(watchdog_deadline("Read", &json!({})).is_none());
        assert!(watchdog_deadline("Write", &json!({})).is_none());
        assert!(watchdog_deadline("Edit", &json!({})).is_none());
        assert!(watchdog_deadline("Agent", &json!({})).is_none());
    }

    #[test]
    fn bash_default_deadline_is_300_plus_grace() {
        let d = watchdog_deadline("Bash", &json!({})).unwrap();
        assert_eq!(d, Duration::from_secs(300 + 30));
    }

    #[test]
    fn bash_custom_timeout_respected() {
        let d = watchdog_deadline("Bash", &json!({"timeout": 60})).unwrap();
        assert_eq!(d, Duration::from_secs(60 + 30));
    }

    #[test]
    fn bash_max_timeout_clamped_to_600() {
        let d = watchdog_deadline("Bash", &json!({"timeout": 99999})).unwrap();
        assert_eq!(d, Duration::from_secs(600 + 30));
    }

    #[test]
    fn timeout_result_carries_stale_reason() {
        let r = timeout_result(Duration::from_secs(120));
        assert!(r.is_error);
        match r.metadata.unwrap() {
            ToolResultMetadata::Stale { reason } => {
                assert_eq!(reason, StaleReason::WatchdogTimeout);
            }
            other => panic!("expected Stale, got {other:?}"),
        }
    }
}
