use std::sync::Arc;

use loopal_config::SandboxPolicy;
use loopal_decision_api::DecisionMode;
use loopal_ipc::Connection;
use loopal_protocol::{AgentEvent, QualifiedAddress, ROOT_AGENT_NAME};
use loopal_tool_api::PermissionMode;
use tokio::sync::{Mutex, mpsc};

use super::super::ProductionWorkflowSpawner;
use super::requests::{causation, request};
use crate::Hub;
use crate::types::{AgentRuntimeFacts, SpawnAuthority};
use crate::workflow::{WorkflowOwner, worker_profile::ResolvedWorkflowWorkerProfile};

#[tokio::test]
async fn read_only_profiles_intersect_root_authority() {
    let root = tempfile::tempdir().unwrap();
    let (events, _event_rx) = mpsc::channel::<AgentEvent>(8);
    let mut hub = Hub::with_cwd(events, root.path().into());
    let (_peer, transport) = loopal_ipc::duplex_pair();
    let connection = Connection::new(transport).into_listening().0;
    let execution = hub
        .registry
        .register_connection_with_parent_execution(
            ROOT_AGENT_NAME,
            connection,
            None,
            Some("trusted-model"),
            None,
        )
        .unwrap();
    let authority = SpawnAuthority {
        model: "trusted-model".into(),
        permission_mode: PermissionMode::AskAnyWrite,
        decision_mode: DecisionMode::Manual,
        sandbox_policy: SandboxPolicy::Disabled,
    };
    let mut facts = AgentRuntimeFacts::root(root.path().into(), authority.clone());
    facts.session_id = Some("session".into());
    assert!(hub.registry.set_runtime_facts(&execution, facts));
    let shutdown_signal = hub.shutdown_signal.clone();
    let spawner = ProductionWorkflowSpawner::new(Arc::new(Mutex::new(hub)), shutdown_signal);

    for (profile, agent_type, sandbox_policy) in [
        (
            ResolvedWorkflowWorkerProfile::Default,
            "default",
            SandboxPolicy::Disabled,
        ),
        (
            ResolvedWorkflowWorkerProfile::Explore,
            "explore",
            SandboxPolicy::ReadOnly,
        ),
        (
            ResolvedWorkflowWorkerProfile::Plan,
            "plan",
            SandboxPolicy::ReadOnly,
        ),
    ] {
        let mut spawn_request = request(causation("wrun", "wnode", "watt"));
        spawn_request.owner =
            WorkflowOwner::new("session", QualifiedAddress::local(ROOT_AGENT_NAME));
        spawn_request.worker_profile = profile;
        let prepared = super::super::spawn_spec::build(&spawner, &spawn_request)
            .await
            .unwrap();
        let params = prepared.start_params("worker-session".into());

        assert_eq!(prepared.agent_type.as_deref(), Some(agent_type));
        assert_eq!(prepared.cwd, root.path());
        assert_eq!(prepared.depth, 1);
        assert_eq!(prepared.authority.model, authority.model);
        assert_eq!(
            prepared.authority.permission_mode,
            authority.permission_mode
        );
        assert_eq!(prepared.authority.decision_mode, authority.decision_mode);
        assert_eq!(prepared.authority.sandbox_policy, sandbox_policy);
        assert_eq!(params.model.as_deref(), Some("trusted-model"));
        assert_eq!(params.permission_mode.as_deref(), Some("ask_any_write"));
        assert_eq!(params.decision_mode.as_deref(), Some("manual"));
        let expected_sandbox = sandbox_policy.to_string();
        assert_eq!(
            params.sandbox_policy.as_deref(),
            Some(expected_sandbox.as_str())
        );
        assert_eq!(params.no_sandbox, sandbox_policy == SandboxPolicy::Disabled);
        assert_eq!(
            params.workflow_completion_result_limit,
            Some(spawn_request.completion_result_limit)
        );
        assert_eq!(
            prepared.workflow_permission_causation.as_ref(),
            Some(&spawn_request.causation)
        );
        assert_eq!(
            prepared
                .runtime_facts(Some("worker-session"))
                .workflow_permission_causation
                .as_ref(),
            Some(&spawn_request.causation)
        );
        assert_eq!(
            prepared
                .runtime_facts(Some("worker-session"))
                .workflow_completion_result_limit,
            Some(spawn_request.completion_result_limit)
        );
    }
}
