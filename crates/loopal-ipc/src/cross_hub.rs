//! Schema constraints shared between Hub / MetaHub / receiver paths
//! for cross-hub spawn requests.
//!
//! Cross-hub spawn cannot share filesystem state with the originating hub,
//! so any filesystem-coupled fields in the payload (cwd, fork_context,
//! session resume) must be rejected — receiver Hub uses its own local
//! state instead.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Authoritative outcome of the MetaHub's destination-side spawn handoff.
///
/// This is deliberately carried as a successful JSON-RPC result. Transport
/// errors cannot distinguish a rejection that happened before side effects
/// from a response lost after the destination committed the child.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum RemoteSpawnOutcome {
    Spawned { response: Value },
    RejectedBeforeSideEffect { message: String },
    OutcomeUnknown { message: String },
}

impl RemoteSpawnOutcome {
    pub fn into_value(self) -> Value {
        serde_json::to_value(self).expect("RemoteSpawnOutcome must serialize")
    }
}

/// Fields that must NOT appear in any cross-hub spawn payload.
/// Caller, MetaHub, and receiver each enforce this independently
/// (defense-in-depth). Adding a new fs-coupled field? Add it here.
pub const FORBIDDEN_SPAWN_FIELDS: &[&str] = &["cwd", "fork_context", "resume"];

/// Reject the payload if it carries any forbidden field. Returns Ok if the
/// payload is clean (or is not a JSON object — the caller decides what to do
/// with malformed payloads at a different layer).
pub fn validate_spawn_payload(params: &Value) -> Result<(), String> {
    for forbidden in FORBIDDEN_SPAWN_FIELDS {
        if params.get(forbidden).is_some() {
            return Err(format!(
                "cross-hub spawn cannot include '{forbidden}' field"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn clean_payload_passes() {
        let p = json!({"name": "child", "prompt": "x"});
        assert!(validate_spawn_payload(&p).is_ok());
    }

    #[test]
    fn rejects_each_forbidden_field() {
        for f in FORBIDDEN_SPAWN_FIELDS {
            let p = json!({"name": "x", *f: "anything"});
            let err = validate_spawn_payload(&p).unwrap_err();
            assert!(err.contains(*f), "error should name '{f}', got: {err}");
        }
    }

    #[test]
    fn non_object_payload_passes_through() {
        // Non-objects don't have keys, so validation is a no-op. Caller's
        // own type check (as_object/.as_str) decides what to do.
        assert!(validate_spawn_payload(&json!(null)).is_ok());
        assert!(validate_spawn_payload(&json!("just-a-string")).is_ok());
        assert!(validate_spawn_payload(&json!([1, 2, 3])).is_ok());
    }

    #[test]
    fn remote_spawn_outcome_round_trips_without_string_classification() {
        for outcome in [
            RemoteSpawnOutcome::Spawned {
                response: json!({"agent_id": "child-id"}),
            },
            RemoteSpawnOutcome::RejectedBeforeSideEffect {
                message: "duplicate name".into(),
            },
            RemoteSpawnOutcome::OutcomeUnknown {
                message: "destination response timed out".into(),
            },
        ] {
            let decoded: RemoteSpawnOutcome =
                serde_json::from_value(outcome.clone().into_value()).unwrap();
            assert_eq!(decoded, outcome);
        }
    }
}
