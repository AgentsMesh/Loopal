//! `/status` command — opens the status dashboard sub-page.

use async_trait::async_trait;

use super::status_config::{build_config_entries, extract_provider_info};
use super::{CommandEffect, CommandHandler};
use crate::app::{
    App, ConfigSnapshot, SessionSnapshot, StatusPageState, StatusTab, SubPage, UsageSnapshot,
};

pub struct StatusCmd;

#[async_trait]
impl CommandHandler for StatusCmd {
    fn name(&self) -> &str {
        "/status"
    }
    fn description(&self) -> &str {
        "Show status dashboard"
    }
    async fn execute(&self, app: &mut App, _arg: Option<&str>) -> CommandEffect {
        open_status_page(app).await;
        CommandEffect::Done
    }
}

async fn open_status_page(app: &mut App) {
    let (mut session, usage) = collect_session_data(app);
    let config = collect_config_snapshot(app);

    // Hub listener port requires async lock — resolve outside the sync session lock.
    if let Some(port) = app.session.hub_listener_port().await {
        session.hub_endpoint = format!("127.0.0.1:{port}");
    }

    app.sub_page = Some(SubPage::StatusPage(StatusPageState {
        active_tab: StatusTab::Status,
        session,
        config,
        usage,
        scroll_offsets: [0; 3],
        filter: String::new(),
        filter_cursor: 0,
    }));
}

/// Extract session/agent data from the locked session state.
fn collect_session_data(app: &App) -> (SessionSnapshot, UsageSnapshot) {
    let state = app.session.lock();
    let conv = state.active_conversation();
    let agent = state
        .agents
        .get(&state.active_view)
        .expect("active_view must exist in agents map");
    let obs = &agent.observable;

    let session = SessionSnapshot {
        session_id: state
            .root_session_id
            .clone()
            .unwrap_or_else(|| "N/A".to_string()),
        cwd: app.cwd.display().to_string(),
        model_display: state.model.clone(),
        mode: state.mode.clone(),
        hub_endpoint: String::new(),
    };

    let usage = UsageSnapshot {
        input_tokens: obs.input_tokens,
        output_tokens: obs.output_tokens,
        context_window: conv.context_window,
        context_used: conv.token_count(),
        turn_count: obs.turn_count,
        tool_count: obs.tool_count,
    };
    (session, usage)
}

/// Load config from disk and build ConfigSnapshot.
fn collect_config_snapshot(app: &App) -> ConfigSnapshot {
    let config = match loopal_config::load_config(&app.cwd) {
        Ok(c) => c,
        Err(_) => {
            return ConfigSnapshot {
                auth_env: String::new(),
                base_url: String::new(),
                mcp_configured: 0,
                mcp_enabled: 0,
                setting_sources: vec!["(failed to load)".to_string()],
                entries: Vec::new(),
            };
        }
    };

    let sources: Vec<String> = config.layers.iter().map(|l| l.to_string()).collect();
    let mcp_configured = config.mcp_servers.len();
    let mcp_enabled = config
        .mcp_servers
        .values()
        .filter(|e| e.config.enabled())
        .count();

    let (auth_env, base_url) = extract_provider_info(&config.settings.providers);
    let entries = build_config_entries(&config.settings);

    ConfigSnapshot {
        auth_env,
        base_url,
        mcp_configured,
        mcp_enabled,
        setting_sources: sources,
        entries,
    }
}
