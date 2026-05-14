use serde::Deserialize;

use crate::executor::RawHookOutput;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct HookOutput {
    pub permission: Option<PermissionOverride>,
    pub additional_context: Option<String>,
    pub updated_input: Option<serde_json::Value>,
    #[serde(default)]
    pub rewake: bool,
    #[serde(default)]
    pub suppress: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionOverride {
    Allow,
    Deny { reason: String },
}

pub fn interpret_output(raw: &RawHookOutput) -> HookOutput {
    if !raw.stdout.is_empty()
        && let Ok(parsed) = serde_json::from_str::<HookOutput>(&raw.stdout)
    {
        return parsed;
    }
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
        _ => HookOutput::default(),
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
}
