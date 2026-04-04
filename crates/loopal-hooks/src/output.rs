//! Structured hook output — typed interpretation of `RawHookOutput`.
//!
//! The `interpret_output` function converts raw exit-code + stdout into
//! a typed `HookOutput`. Consumers read only the fields relevant to
//! their event (ISP). Backward compatible with exit-code 0/2 protocol.

use serde::Deserialize;

use crate::executor::RawHookOutput;

/// Interpreted hook output. Each field is optional — consumers only
/// read what their event requires (ISP).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct HookOutput {
    /// PreToolUse: override permission decision.
    pub permission: Option<PermissionOverride>,
    /// Text to inject into the conversation (tool result or LLM context).
    pub additional_context: Option<String>,
    /// PreToolUse: replace tool input parameters.
    pub updated_input: Option<serde_json::Value>,
    /// Signal to wake an idle agent (used by asyncRewake hooks).
    #[serde(default)]
    pub rewake: bool,
    /// Suppress the default behavior for this event.
    #[serde(default)]
    pub suppress: bool,
}

/// Permission override returned by PreToolUse hooks.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionOverride {
    Allow,
    Deny { reason: String },
}

/// Interpret raw executor output into typed `HookOutput`.
///
/// **Strategy:** Try JSON parse first (structured output). On parse failure,
/// fall back to the exit-code protocol (backward compatible).
///
/// Information Expert: this function knows both the JSON schema and the
/// exit-code protocol, so interpretation logic lives here.
pub fn interpret_output(raw: &RawHookOutput) -> HookOutput {
    // Structured path: try to parse stdout as JSON
    if !raw.stdout.is_empty() && let Ok(parsed) = serde_json::from_str::<HookOutput>(&raw.stdout) {
        return parsed;
    }
    // Fallback: exit-code protocol
    match raw.exit_code {
        0 => HookOutput::default(),
        2 => {
            let text = if raw.stdout.is_empty() {
                raw.stderr.clone()
            } else {
                raw.stdout.clone()
            };
            HookOutput {
                additional_context: if text.is_empty() { None } else { Some(text) },
                rewake: true,
                ..Default::default()
            }
        }
        _ => HookOutput::default(), // non-zero non-2: logged by caller, no action
    }
}

/// Interpret raw output for PreToolUse hooks (backward-compat: non-zero = deny).
///
/// PreToolUse hooks historically treated any non-zero exit code as rejection.
/// This variant preserves that behavior while adding structured JSON support.
pub fn interpret_pre_tool_output(raw: &RawHookOutput) -> HookOutput {
    // Structured path: try JSON first
    if !raw.stdout.is_empty() && let Ok(parsed) = serde_json::from_str::<HookOutput>(&raw.stdout) {
        return parsed;
    }
    match raw.exit_code {
        0 => HookOutput::default(),
        _ => HookOutput {
            permission: Some(PermissionOverride::Deny {
                reason: if raw.stderr.is_empty() {
                    format!("hook exited with code {}", raw.exit_code)
                } else {
                    raw.stderr.trim().to_string()
                },
            }),
            ..Default::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_output_parsed() {
        let raw = RawHookOutput {
            exit_code: 0,
            stdout: r#"{"additional_context":"lint passed","rewake":false}"#.into(),
            stderr: String::new(),
        };
        let out = interpret_output(&raw);
        assert_eq!(out.additional_context.as_deref(), Some("lint passed"));
        assert!(!out.rewake);
    }

    #[test]
    fn exit_code_2_fallback() {
        let raw = RawHookOutput {
            exit_code: 2,
            stdout: "error: type mismatch".into(),
            stderr: String::new(),
        };
        let out = interpret_output(&raw);
        assert_eq!(
            out.additional_context.as_deref(),
            Some("error: type mismatch")
        );
        assert!(out.rewake);
    }

    #[test]
    fn exit_code_0_noop() {
        let raw = RawHookOutput {
            exit_code: 0,
            stdout: "not json".into(),
            stderr: String::new(),
        };
        let out = interpret_output(&raw);
        assert!(out.additional_context.is_none());
        assert!(out.permission.is_none());
    }

    #[test]
    fn exit_code_1_noop() {
        let raw = RawHookOutput {
            exit_code: 1,
            stdout: String::new(),
            stderr: "hook crashed".into(),
        };
        let out = interpret_output(&raw);
        assert!(out.additional_context.is_none());
    }

    // ── interpret_pre_tool_output tests ─────────────────────

    #[test]
    fn pre_tool_nonzero_exit_denies() {
        let raw = RawHookOutput {
            exit_code: 1,
            stdout: String::new(),
            stderr: "denied by hook".into(),
        };
        let out = interpret_pre_tool_output(&raw);
        assert!(matches!(out.permission, Some(PermissionOverride::Deny { .. })));
        if let Some(PermissionOverride::Deny { reason }) = out.permission {
            assert!(reason.contains("denied by hook"));
        }
    }

    #[test]
    fn pre_tool_exit_0_allows() {
        let raw = RawHookOutput {
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
        };
        let out = interpret_pre_tool_output(&raw);
        assert!(out.permission.is_none());
    }

    #[test]
    fn pre_tool_json_structured_deny() {
        let raw = RawHookOutput {
            exit_code: 0,
            stdout: r#"{"permission":{"deny":{"reason":"policy violation"}}}"#.into(),
            stderr: String::new(),
        };
        let out = interpret_pre_tool_output(&raw);
        assert!(matches!(out.permission, Some(PermissionOverride::Deny { .. })));
    }

    #[test]
    fn pre_tool_json_structured_allow() {
        let raw = RawHookOutput {
            exit_code: 0,
            stdout: r#"{"permission":"allow"}"#.into(),
            stderr: String::new(),
        };
        let out = interpret_pre_tool_output(&raw);
        assert!(matches!(out.permission, Some(PermissionOverride::Allow)));
    }
}
