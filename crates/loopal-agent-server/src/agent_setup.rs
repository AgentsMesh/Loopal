use crate::agent_setup_context::AgentSetupContext;
use crate::agent_setup_helpers::{
    build_microcompact_idle, collect_feature_tags, spawn_sub_agent_forwarder, thinking_inputs,
};
use crate::agent_setup_prompt::{SessionPromptOptions, build_session_system_prompt};
use crate::params::AgentSetupResult;
use loopal_agent::shared::{AgentShared, SchedulerHandle};
use loopal_context::ContextBudget;
use std::sync::Arc;

pub async fn build_with_frontend(ctx: AgentSetupContext<'_>) -> anyhow::Result<AgentSetupResult> {
    let AgentSetupContext {
        cwd,
        config,
        start,
        frontend,
        interrupt,
        interrupt_tx,
        kernel,
        hub_connection,
        session_dir_override,
        hub,
        decision_context,
        decision_cell,
        session_id,
        router,
    } = ctx;
    let model = router.model();
    let permission_mode = config.settings.permission_mode;
    let (thinking_config, thinking_recommendation) = thinking_inputs(config);
    let thinking_state = loopal_provider_api::SharedThinkingConfig::new(thinking_config.clone());
    let (mode, mode_str) = match start.mode.as_deref() {
        Some("plan") => (loopal_runtime::AgentMode::Plan, "plan"),
        _ => (loopal_runtime::AgentMode::Act, "act"),
    };
    let depth = start.depth.unwrap_or(0);
    let (session_manager, session, initial_turns) =
        crate::agent_setup_session::open(cwd, &model, session_id, session_dir_override, start)?;
    let recover_workflows = crate::agent_setup_workflow::should_recover_workflows(
        start.lifecycle,
        depth,
        &config.settings.workflow,
    );
    let recovered_workflows =
        recover_pending_workflows(recover_workflows, &session_manager, &session.id)?;
    let workflow_lease_tracker = Arc::new(loopal_runtime::WorkflowLeaseTracker::recovered(
        &initial_turns,
        recovered_workflows,
    ));
    let event_tx = spawn_sub_agent_forwarder(frontend.clone());

    let crate::session_resources::SessionScopedResources {
        task_store,
        scheduler,
        resume_hooks,
    } = crate::session_resources::build_session_scoped_resources(
        hub,
        crate::session_resources::resolve_sessions_root(session_dir_override),
        &session.id,
        depth,
    )
    .await?;
    let (scheduler_handle, scheduled_rx) =
        SchedulerHandle::create_with_scheduler(scheduler.clone());
    let message_snapshot = Arc::new(std::sync::RwLock::new(Vec::new()));
    let protected_effect_audit = crate::protected_effect_audit::client(hub_connection.clone());
    let goal_session = at_root(depth, || {
        let goal_store = session_manager.goal_store();
        std::sync::Arc::new(loopal_runtime::GoalRuntimeSession::new(
            session.id.clone(),
            goal_store,
            frontend.event_emitter(),
        ))
    });
    let workflow_control = crate::agent_setup_workflow::build_control(
        depth,
        &config.settings.workflow,
        hub_connection.clone(),
        workflow_lease_tracker.clone(),
    );
    let agent_shared = Arc::new(AgentShared {
        kernel: kernel.clone(),
        task_store: task_store.clone(),
        hub_connection,
        cwd: cwd.to_path_buf(),
        depth,
        agent_name: "main".into(),
        parent_event_tx: Some(event_tx),
        cancel_token: None,
        scheduler_handle,
        message_snapshot: message_snapshot.clone(),
        goal_session: goal_session.clone(),
        workflow_control: workflow_control.clone(),
    });

    let memory_channel = crate::memory_adapter::build_memory_channel(
        start.lifecycle == loopal_runtime::LifecycleMode::Persistent,
        &config.settings,
        &agent_shared,
        &model,
    );

    let services = crate::agent_setup_workflow::services(&agent_shared, thinking_state.reader());
    let workflow_input_handler = crate::agent_setup_workflow::build_input_handler_with_model_router(
        depth,
        &config.settings.workflow,
        workflow_control.clone(),
        services.one_shot_chat.clone(),
        router.reader(),
        thinking_recommendation.clone(),
    );
    let tool_defs = kernel.tool_definitions();
    let features = collect_feature_tags(config, memory_channel.is_some());
    let system_prompt = build_session_system_prompt(
        config,
        &kernel,
        cwd,
        SessionPromptOptions {
            mode: mode_str,
            agent_type: start.agent_type.as_deref(),
            depth: start.depth.unwrap_or(0),
            tool_defs: &tool_defs,
            features,
        },
    )
    .await;

    let tool_tokens = ContextBudget::estimate_tool_tokens(&tool_defs);
    let budget = loopal_runtime::build_initial_budget(
        &model,
        config.settings.max_context_tokens,
        &system_prompt,
        tool_tokens,
    );
    let lifecycle = start.lifecycle;
    let tool_filter = crate::spawn_policy::build_depth_tool_filter(
        depth,
        config.settings.harness.agent_max_depth,
    );

    let params = crate::agent_loop_params_factory::assemble_agent_loop_params(
        crate::agent_loop_params_factory::AgentLoopAssembly {
            config: loopal_runtime::AgentConfig {
                lifecycle,
                router,
                system_prompt,
                mode,
                permission_mode,
                tool_filter,
                thinking_config,
                thinking_state: Some(thinking_state),
                workflow_preset_thinking_recommendation: thinking_recommendation,
                context_tokens_cap: config.settings.max_context_tokens,
                microcompact_idle: build_microcompact_idle(&config.settings.compaction),
                plan_state: None,
            },
            deps: loopal_runtime::AgentDeps {
                kernel,
                frontend,
                session_manager,
                decision_context,
                protected_effect_audit,
            },
            session,
            hydrate_initial_history: start.resume.is_some() || !initial_turns.is_empty(),
            initial_turns,
            budget,
            interrupt,
            interrupt_tx,
            shared: services.shared,
            scheduled_rx,
            harness: config.settings.harness.clone(),
            message_snapshot,
            resume_hooks,
            memory_channel,
            one_shot_chat: Some(services.one_shot_chat),
            fetch_refiner_policy: Some(services.fetch_refiner),
            outstanding_tasks: Some(services.outstanding_tasks),
            goal_session,
            scheduler: scheduler.clone(),
            workflow_permission_causation: start.workflow_permission_causation.clone(),
            decision_cell,
            workflow_input_handler,
            workflow_lease_tracker,
        },
    );
    Ok(AgentSetupResult {
        params,
        task_store,
        scheduler,
        agent_shared,
    })
}

fn recover_pending_workflows(
    recover: bool,
    session_manager: &loopal_runtime::SessionManager,
    session_id: &str,
) -> anyhow::Result<Vec<loopal_protocol::WorkflowRunId>> {
    if recover {
        Ok(session_manager.pending_workflow_delivery_run_ids(session_id)?)
    } else {
        Ok(Vec::new())
    }
}

fn at_root<T>(depth: u32, build: impl FnOnce() -> T) -> Option<T> {
    if depth == 0 { Some(build()) } else { None }
}

#[cfg(test)]
mod tests {
    use super::{at_root, recover_pending_workflows};

    #[test]
    fn recovery_switches_between_pending_journal_scan_and_empty_fast_path() {
        let temp = tempfile::tempdir().unwrap();
        let manager = loopal_runtime::SessionManager::with_base_dir(temp.path().join("state"));
        let session = manager
            .create_session(temp.path(), "test-model")
            .expect("create test session");

        assert!(
            recover_pending_workflows(false, &manager, &session.id)
                .unwrap()
                .is_empty()
        );
        assert!(
            recover_pending_workflows(true, &manager, &session.id)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn goal_session_builder_is_root_only() {
        assert_eq!(at_root(0, || "root"), Some("root"));
        assert_eq!(at_root(1, || "child"), None);
    }
}
