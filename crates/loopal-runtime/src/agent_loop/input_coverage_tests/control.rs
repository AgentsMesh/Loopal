use loopal_protocol::ControlCommand;
use loopal_tool_api::PermissionMode;

use super::support::make_fixture;
use crate::AgentMode;
use crate::agent_loop::PlanModeState;
use crate::agent_loop::input_control::ControlOutcome;

fn assert_rejected(outcome: ControlOutcome, expected: &str) {
    assert!(matches!(
        outcome,
        ControlOutcome::Rejected(reason) if reason.contains(expected)
    ));
}

#[tokio::test]
async fn control_handlers_cover_rejections_clear_and_runtime_configuration() {
    let mut fixture = make_fixture();
    fixture.runner.turn_count = 9;
    assert!(matches!(
        fixture
            .runner
            .handle_control(ControlCommand::Clear)
            .await
            .unwrap(),
        ControlOutcome::Applied {
            continuation: false
        }
    ));
    assert_eq!(fixture.runner.turn_count, 0);
    assert_rejected(
        fixture
            .runner
            .handle_control(ControlCommand::ModelSwitch(" \t".into()))
            .await
            .unwrap(),
        "must not be empty",
    );
    assert_rejected(
        fixture
            .runner
            .handle_control(ControlCommand::Rewind { turn_index: 42 })
            .await
            .unwrap(),
        "does not exist",
    );

    let not_directory = fixture.temp.path().join("not-directory");
    std::fs::write(&not_directory, b"file").unwrap();
    crate::agent_loop::input_control::persist_local_setting(
        not_directory.to_str().unwrap(),
        "model",
        serde_json::json!("ignored"),
    );

    assert!(matches!(
        fixture
            .runner
            .handle_permission_switch("bypass".into())
            .await
            .unwrap(),
        ControlOutcome::Applied { .. }
    ));
    assert!(fixture.runner.params.config.plan_state.is_none());

    assert_rejected(
        fixture
            .runner
            .handle_permission_switch("invalid".into())
            .await
            .unwrap(),
        "invalid permission mode",
    );
    fixture.runner.params.config.plan_state = Some(PlanModeState {
        previous_mode: AgentMode::Act,
        previous_permission_mode: PermissionMode::Bypass,
        tool_filter: Default::default(),
    });
    assert!(matches!(
        fixture
            .runner
            .handle_permission_switch("ask_any_write".into())
            .await
            .unwrap(),
        ControlOutcome::Applied { .. }
    ));
    assert_eq!(
        fixture
            .runner
            .params
            .config
            .plan_state
            .as_ref()
            .unwrap()
            .previous_permission_mode,
        PermissionMode::AskAnyWrite
    );

    assert_rejected(
        fixture
            .runner
            .handle_decision_switch("invalid".into())
            .await
            .unwrap(),
        "invalid decision mode",
    );
    assert_rejected(
        fixture
            .runner
            .handle_decision_switch("agent".into())
            .await
            .unwrap(),
        "not implemented",
    );
    assert!(matches!(
        fixture
            .runner
            .handle_decision_switch("classifier".into())
            .await
            .unwrap(),
        ControlOutcome::Applied { .. }
    ));
    assert_eq!(
        fixture.runner.params.decision_cell.get(),
        loopal_decision_api::DecisionMode::Classifier
    );

    assert_rejected(
        fixture
            .runner
            .handle_sandbox_switch("invalid".into())
            .await
            .unwrap(),
        "invalid sandbox policy",
    );
    assert!(matches!(
        fixture
            .runner
            .handle_sandbox_switch("read_only".into())
            .await
            .unwrap(),
        ControlOutcome::Applied { .. }
    ));
    assert_eq!(
        fixture.runner.params.deps.kernel.sandbox_policy(),
        loopal_config::SandboxPolicy::ReadOnly
    );
}
