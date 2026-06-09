use async_trait::async_trait;

use super::runtime_config_picker::{EnumOption, open_enum_picker};
use super::{CommandEffect, CommandHandler};
use crate::app::{App, EnumPickerKind};

pub struct PermissionCmd;

#[async_trait]
impl CommandHandler for PermissionCmd {
    fn name(&self) -> &str {
        "/permission"
    }
    fn description(&self) -> &str {
        "Switch permission mode (session only)"
    }
    fn has_arg(&self) -> bool {
        false
    }
    async fn execute(&self, app: &mut App, _arg: Option<&str>) -> CommandEffect {
        let current = app.active_observable().permission_mode;
        let default = loopal_tool_api::PermissionMode::default().to_string();
        open_enum_picker(
            app,
            "Switch Permission Mode",
            EnumPickerKind::Permission,
            &current,
            &default,
            &[
                EnumOption {
                    label: "Bypass",
                    description: "bypass — never ask",
                    value: "bypass",
                },
                EnumOption {
                    label: "Ask Dangerous",
                    description: "ask_dangerous — ask on dangerous tools",
                    value: "ask_dangerous",
                },
                EnumOption {
                    label: "Ask Any Write",
                    description: "ask_any_write — ask on any write",
                    value: "ask_any_write",
                },
            ],
        );
        CommandEffect::Done
    }
}

pub struct DecisionCmd;

#[async_trait]
impl CommandHandler for DecisionCmd {
    fn name(&self) -> &str {
        "/decision"
    }
    fn description(&self) -> &str {
        "Switch decision mode (session only)"
    }
    fn has_arg(&self) -> bool {
        false
    }
    async fn execute(&self, app: &mut App, _arg: Option<&str>) -> CommandEffect {
        let current = app.active_observable().decision_mode;
        let default = loopal_decision_api::DecisionMode::default().to_string();
        open_enum_picker(
            app,
            "Switch Decision Mode",
            EnumPickerKind::Decision,
            &current,
            &default,
            &[
                EnumOption {
                    label: "Manual",
                    description: "manual — you answer every prompt",
                    value: "manual",
                },
                EnumOption {
                    label: "Classifier",
                    description: "classifier — LLM races you",
                    value: "classifier",
                },
                EnumOption {
                    label: "Agent",
                    description: "agent — falls back to classifier",
                    value: "agent",
                },
            ],
        );
        CommandEffect::Done
    }
}

pub struct SandboxCmd;

#[async_trait]
impl CommandHandler for SandboxCmd {
    fn name(&self) -> &str {
        "/sandbox"
    }
    fn description(&self) -> &str {
        "Switch sandbox policy (session only)"
    }
    fn has_arg(&self) -> bool {
        false
    }
    async fn execute(&self, app: &mut App, _arg: Option<&str>) -> CommandEffect {
        let current = app.active_observable().sandbox_policy;
        let default = loopal_config::SandboxPolicy::default().to_string();
        open_enum_picker(
            app,
            "Switch Sandbox Policy",
            EnumPickerKind::Sandbox,
            &current,
            &default,
            &[
                EnumOption {
                    label: "Disabled",
                    description: "disabled — no sandbox",
                    value: "disabled",
                },
                EnumOption {
                    label: "Default Write",
                    description: "default_write — writes allowed, gated",
                    value: "default_write",
                },
                EnumOption {
                    label: "Read Only",
                    description: "read_only — all writes blocked",
                    value: "read_only",
                },
            ],
        );
        CommandEffect::Done
    }
}
