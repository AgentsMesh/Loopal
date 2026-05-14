use crate::executor::RawHookOutput;
use crate::output::{HookOutput, PermissionOverride};

pub fn interpret_pre_tool_output(raw: &RawHookOutput) -> HookOutput {
    if !raw.stdout.is_empty()
        && let Ok(parsed) = serde_json::from_str::<HookOutput>(&raw.stdout)
    {
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
    fn pre_tool_nonzero_exit_denies() {
        let raw = RawHookOutput {
            exit_code: 1,
            stdout: String::new(),
            stderr: "denied by hook".into(),
        };
        let out = interpret_pre_tool_output(&raw);
        assert!(matches!(
            out.permission,
            Some(PermissionOverride::Deny { .. })
        ));
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
        assert!(matches!(
            out.permission,
            Some(PermissionOverride::Deny { .. })
        ));
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
