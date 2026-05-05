//! Internal agent loop setup — builds `AgentLoopParams` from resolved config.
use crate::agent_setup_context::AgentSetupContext;
use crate::agent_setup_helpers::{
    build_initial_messages, collect_feature_tags, spawn_sub_agent_forwarder,
};
use crate::params::AgentSetupResult;
use loopal_agent::shared::{AgentShared, SchedulerHandle};
use loopal_context::ContextBudget;
use loopal_context::system_prompt::build_system_prompt;
use std::sync::Arc;

/// Build `AgentLoopParams` with a pre-constructed frontend.
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
    } = ctx;
    let router = loopal_provider_api::ModelRouter::from_parts(
        config.settings.model.clone(),
        config.settings.model_routing.clone(),
    );
    let model = router
        .resolve(loopal_provider_api::TaskType::Default)
        .to_string();
    let permission_mode = config.settings.permission_mode;
    let thinking_config = config.settings.thinking.clone();
    let (mode, mode_str) = match start.mode.as_deref() {
        Some("plan") => (loopal_runtime::AgentMode::Plan, "plan"),
        _ => (loopal_runtime::AgentMode::Act, "act"),
    };

    let session_manager = if let Some(dir) = session_dir_override {
        loopal_runtime::SessionManager::with_base_dir(dir.to_path_buf())
    } else {
        loopal_runtime::SessionManager::new()?
    };
    let (session, resume_messages) = if let Some(ref sid) = start.resume {
        let (s, msgs) = session_manager.resume_session(sid)?;
        (s, msgs)
    } else {
        (session_manager.create_session(cwd, &model)?, Vec::new())
    };

    let event_tx = spawn_sub_agent_forwarder(frontend.clone());

    // Build task store + scheduler + resume hooks. Scheduler's async
    // bind to the session id runs in `session_start::run` after this
    // synchronous builder returns (covers both fresh and resumed
    // sessions through one path).
    let depth = start.depth.unwrap_or(0);
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
    });

    let memory_channel = crate::memory_adapter::build_memory_channel(
        start.lifecycle == loopal_runtime::LifecycleMode::Persistent,
        &config.settings,
        &agent_shared,
        &model,
    );

    let auto_classifier = (permission_mode == loopal_tool_api::PermissionMode::Auto).then(|| {
        Arc::new(loopal_auto_mode::AutoClassifier::new_with_thresholds(
            config.instructions.clone(),
            cwd.to_string_lossy().into_owned(),
            config.settings.harness.cb_max_consecutive_denials,
            config.settings.harness.cb_max_total_denials,
        ))
    });
    let shared_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(agent_shared.clone());
    let one_shot_chat: Arc<dyn loopal_tool_api::OneShotChatService> = agent_shared.clone();
    let fetch_refiner_policy: Arc<dyn loopal_tool_api::FetchRefinerPolicy> = agent_shared.clone();
    let skills: Vec<_> = config.skills.values().map(|e| e.skill.clone()).collect();
    let skills_summary = loopal_config::format_skills_summary(&skills);
    let tool_defs = kernel.tool_definitions();

    let features = collect_feature_tags(config, memory_channel.is_some());

    let mut system_prompt = build_system_prompt(
        &config.instructions,
        &tool_defs,
        mode_str,
        &cwd.to_string_lossy(),
        &skills_summary,
        &config.memory,
        start.agent_type.as_deref(),
        features,
        start.depth.unwrap_or(0),
    );
    crate::prompt_post::append_runtime_sections(&mut system_prompt, &kernel);

    let messages = build_initial_messages(resume_messages, start);

    let tool_tokens = ContextBudget::estimate_tool_tokens(&tool_defs);
    let budget = loopal_runtime::build_initial_budget(
        &model,
        config.settings.max_context_tokens,
        &system_prompt,
        tool_tokens,
    );
    let lifecycle = start.lifecycle;
    let depth = start.depth.unwrap_or(0);
    let tool_filter = crate::spawn_policy::build_depth_tool_filter(
        &kernel,
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
                context_tokens_cap: config.settings.max_context_tokens,
                plan_state: None,
            },
            deps: loopal_runtime::AgentDeps {
                kernel,
                frontend,
                session_manager,
            },
            session,
            messages,
            budget,
            interrupt,
            interrupt_tx,
            shared: shared_any,
            scheduled_rx,
            harness: config.settings.harness.clone(),
            message_snapshot,
            resume_hooks,
            memory_channel,
            auto_classifier,
            one_shot_chat: Some(one_shot_chat),
            fetch_refiner_policy: Some(fetch_refiner_policy),
        },
    );
    Ok(AgentSetupResult {
        params,
        task_store,
        scheduler,
        agent_shared,
    })
}
