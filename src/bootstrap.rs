use std::sync::Arc;

use clap::Parser;
use tokio::sync::mpsc;

use loopagent_config::{load_instructions, load_settings, load_skills};
use loopagent_context::ContextPipeline;
use loopagent_context::middleware::{ContextGuard, SmartCompact, TurnLimit};
use loopagent_context::system_prompt::build_system_prompt;
use loopagent_kernel::Kernel;
use loopagent_runtime::{AgentLoopParams, AgentMode, SessionManager, agent_loop};
use loopagent_tui::command::merge_commands;
use loopagent_types::command::UserCommand;
use loopagent_types::event::AgentEvent;

use crate::cli::Cli;

pub async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Load config
    let cwd = std::env::current_dir()?;
    let mut settings = load_settings(&cwd)?;

    // CLI overrides
    if let Some(model) = &cli.model {
        settings.model = model.clone();
    }
    if let Some(perm) = &cli.permission {
        settings.permission_mode = match perm.as_str() {
            "accept-edits" => loopagent_types::permission::PermissionMode::AcceptEdits,
            "bypass" | "yolo" => loopagent_types::permission::PermissionMode::BypassPermissions,
            "plan" => loopagent_types::permission::PermissionMode::Plan,
            _ => loopagent_types::permission::PermissionMode::Default,
        };
    }
    if cli.plan {
        settings.permission_mode = loopagent_types::permission::PermissionMode::Plan;
    }

    let model = settings.model.clone();
    let max_turns = settings.max_turns;
    let permission_mode = settings.permission_mode;
    let mode = if cli.plan {
        AgentMode::Plan
    } else {
        AgentMode::Act
    };
    let mode_str = if cli.plan { "plan" } else { "act" }.to_string();

    tracing::info!(model = %model, mode = %mode_str, "starting");

    // Build kernel
    let mut kernel = Kernel::new(settings)?;
    kernel.start_mcp().await?;
    let kernel = Arc::new(kernel);

    // Session management
    let session_manager = SessionManager::new()?;
    let (session, mut messages) = if let Some(ref session_id) = cli.resume {
        session_manager.resume_session(session_id)?
    } else {
        let session = session_manager.create_session(&cwd, &model)?;
        (session, Vec::new())
    };

    // If initial prompt provided, add as first user message
    if !cli.prompt.is_empty() {
        let prompt_text = cli.prompt.join(" ");
        messages.push(loopagent_types::message::Message::user(&prompt_text));
    }

    // Load skills and build summary for system prompt
    let skills = load_skills(&cwd);
    let skills_summary = format_skills_summary(&skills);
    let commands = merge_commands(&skills);

    // Build system prompt
    let instructions = load_instructions(&cwd)?;
    let tool_defs = kernel.tool_definitions();
    // NOTE: mode suffix is appended in agent_loop per-turn, not here
    let system_prompt = build_system_prompt(
        &instructions,
        &tool_defs,
        "",
        &cwd.to_string_lossy(),
        &skills_summary,
    );

    // Channels
    let (agent_event_tx, agent_event_rx) = mpsc::channel::<AgentEvent>(256);
    let (user_input_tx, user_input_rx) = mpsc::channel::<UserCommand>(16);
    let (permission_decision_tx, permission_decision_rx) = mpsc::channel::<bool>(16);

    // Build context pipeline with middlewares
    let mut context_pipeline = ContextPipeline::new();
    context_pipeline.add(Box::new(TurnLimit::new(max_turns)));
    context_pipeline.add(Box::new(ContextGuard));
    context_pipeline.add(Box::new(SmartCompact::new(10)));

    // Spawn agent runtime loop
    let agent_params = AgentLoopParams {
        kernel: kernel.clone(),
        session: session.clone(),
        messages,
        model: model.clone(),
        system_prompt,
        mode,
        permission_mode,
        max_turns,
        event_tx: agent_event_tx.clone(),
        input_rx: user_input_rx,
        permission_rx: permission_decision_rx,
        session_manager,
        context_pipeline,
    };

    tokio::spawn(async move {
        if let Err(e) = agent_loop(agent_params).await {
            tracing::error!(error = %e, "agent loop error");
        }
    });

    // Bridge: TUI sends permission decisions as (id, bool), but agent_loop
    // expects a plain bool on permission_rx. Bridge the two channels.
    let permission_bridge_tx = permission_decision_tx;
    let (tui_permission_tx, mut tui_permission_rx) = mpsc::channel::<(String, bool)>(16);

    // NOTE: We drop the tool ID here because agent_loop processes tool_uses
    // sequentially (one at a time), so there is only ever one pending permission
    // request. If parallel tool execution is added in the future, this bridge
    // must be updated to route approvals by ID.
    tokio::spawn(async move {
        while let Some((_id, approved)) = tui_permission_rx.recv().await {
            let _ = permission_bridge_tx.send(approved).await;
        }
    });

    // Launch TUI
    loopagent_tui::run_tui(
        model,
        mode_str,
        commands,
        cwd,
        agent_event_rx,
        user_input_tx,
        tui_permission_tx,
    )
    .await?;

    tracing::info!("shutting down");

    Ok(())
}

/// Format a skills summary section for the system prompt.
/// Returns empty string when no skills are loaded.
fn format_skills_summary(skills: &[loopagent_config::Skill]) -> String {
    if skills.is_empty() {
        return String::new();
    }
    let mut section = String::from(
        "# Available Skills\nUser can invoke these via /name:\n",
    );
    for skill in skills {
        section.push_str(&format!("- {}: {}\n", skill.name, skill.description));
    }
    section
}
