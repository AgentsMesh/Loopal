//! Shared bootstrap logic — creates Hub + spawns root agent.
//!
//! Used by both `multiprocess` (TUI mode) and `acp` (IDE mode) bootstrap paths.

use std::sync::Arc;

use tokio::sync::{Mutex, mpsc};
use tracing::info;

use loopal_agent_hub::Hub;
use loopal_agent_hub::hub_server;
use loopal_protocol::AgentEvent;

use crate::cli::Cli;

/// Context returned after Hub + root agent bootstrap.
pub struct BootstrapContext {
    pub hub: Arc<Mutex<Hub>>,
    pub event_rx: mpsc::Receiver<AgentEvent>,
    pub agent_proc: loopal_agent_client::AgentProcess,
    /// Root agent's session ID (for sub-agent ref persistence).
    pub root_session_id: String,
    /// TCP listener token — printed on stderr so external clients can
    /// `--attach-hub` this Hub.
    pub hub_token: String,
}

/// Create Hub, start TCP listener, spawn root agent, register as "main".
pub async fn bootstrap_hub_and_agent(
    cli: &Cli,
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
    resume: Option<&str>,
) -> anyhow::Result<BootstrapContext> {
    let (event_tx, event_rx) = mpsc::channel(256);
    let hub = Arc::new(Mutex::new(Hub::with_cwd(event_tx, cwd.to_path_buf())));
    hub.lock().await.max_total_agents = config.settings.harness.agent_max_total;

    let (listener, port, hub_token) = hub_server::start_hub_listener(hub.clone()).await?;
    {
        let mut h = hub.lock().await;
        h.listener_port = Some(port);
        h.listener_token = Some(hub_token.clone());
    }
    let hub_accept = hub.clone();
    let token_for_loop = hub_token.clone();
    tokio::spawn(async move {
        hub_server::accept_loop(listener, hub_accept, token_for_loop).await;
    });

    if let Some(ref meta_addr) = cli.child.join_hub {
        super::uplink_bootstrap::connect_to_meta_hub(
            &hub,
            meta_addr,
            cli.child.hub_name.as_deref(),
        )
        .await?;
    }

    let agent_proc = loopal_agent_client::AgentProcess::spawn(None).await?;
    let client = loopal_agent_client::AgentClient::new(agent_proc.transport());
    client.initialize().await?;

    let mode_str = if cli.child.plan { "plan" } else { "act" };
    let prompt = if cli.prompt.is_empty() {
        None
    } else {
        Some(cli.prompt.join(" "))
    };
    let lifecycle_str = if cli.child.ephemeral {
        Some("ephemeral")
    } else {
        None // default: persistent (server decides based on prompt)
    };
    let permission_argv = build_permission_argv(
        cli.child.permission.as_deref(),
        cli.child.decision.as_deref(),
    );
    let root_session_id = client
        .start_agent(&loopal_agent_client::StartAgentParams {
            cwd: cwd.to_path_buf(),
            model: Some(config.settings.model.clone()),
            mode: Some(mode_str.to_string()),
            prompt: prompt.clone(),
            permission: permission_argv,
            no_sandbox: cli.child.no_sandbox,
            resume: resume.map(String::from),
            lifecycle: lifecycle_str.map(String::from),
            agent_type: None,
            depth: None,
            fork_context: None,
        })
        .await?;

    let (root_conn, incoming_rx) = client.into_parts();
    loopal_agent_hub::agent_io::start_agent_io(hub.clone(), "main", root_conn, incoming_rx);
    info!("root agent registered as 'main' in Hub");

    Ok(BootstrapContext {
        hub,
        event_rx,
        agent_proc,
        root_session_id,
        hub_token,
    })
}

/// JSON-encode the `permission` spawn argv from `(--permission, --decision)` CLI inputs.
///
/// Returns `None` only when both flags are absent so we don't override server-side
/// resolved settings. When either flag is set, both fields fall back to defaults
/// (`AskAnyWrite` / `Manual`) so the receiver gets a well-formed `{mode, decision}`.
/// `yolo` is normalized to `bypass` before parsing.
fn build_permission_argv(perm: Option<&str>, decision: Option<&str>) -> Option<String> {
    if perm.is_none() && decision.is_none() {
        return None;
    }
    let mode = perm
        .map(|p| if p == "yolo" { "bypass" } else { p })
        .and_then(|s| s.parse::<loopal_tool_api::PermissionMode>().ok())
        .unwrap_or(loopal_tool_api::PermissionMode::AskAnyWrite);
    let decision = decision
        .and_then(|s| s.parse::<loopal_decision_api::DecisionMode>().ok())
        .unwrap_or(loopal_decision_api::DecisionMode::Manual);
    Some(
        serde_json::json!({
            "mode": mode,
            "decision": decision,
        })
        .to_string(),
    )
}

#[cfg(test)]
mod build_permission_argv_tests {
    use super::build_permission_argv;

    #[test]
    fn none_none_returns_none() {
        assert_eq!(build_permission_argv(None, None), None);
    }

    #[test]
    fn permission_only_fills_default_manual_decision() {
        let s = build_permission_argv(Some("ask_dangerous"), None).unwrap();
        assert!(
            s.contains(r#""mode":"ask_dangerous""#),
            "mode must be passed through: {s}"
        );
        assert!(
            s.contains(r#""decision":"manual""#),
            "decision must default to manual: {s}"
        );
    }

    #[test]
    fn decision_only_fills_default_ask_any_write_mode() {
        let s = build_permission_argv(None, Some("classifier")).unwrap();
        assert!(
            s.contains(r#""decision":"classifier""#),
            "classifier decision must serialize: {s}"
        );
        assert!(
            s.contains(r#""mode":"ask_any_write""#),
            "mode must default to ask_any_write: {s}"
        );
    }

    #[test]
    fn both_provided_encodes_both() {
        let s = build_permission_argv(Some("bypass"), Some("classifier")).unwrap();
        assert!(s.contains(r#""mode":"bypass""#));
        assert!(s.contains(r#""decision":"classifier""#));
    }

    #[test]
    fn yolo_alias_normalized_to_bypass() {
        let s = build_permission_argv(Some("yolo"), Some("manual")).unwrap();
        assert!(
            s.contains(r#""mode":"bypass""#),
            "yolo must normalize to bypass: {s}"
        );
        assert!(s.contains(r#""decision":"manual""#));
    }

    #[test]
    fn unknown_permission_falls_back_to_ask_any_write() {
        let s = build_permission_argv(Some("garbage"), Some("classifier")).unwrap();
        assert!(
            s.contains(r#""mode":"ask_any_write""#),
            "garbage perm should fall back to ask_any_write: {s}"
        );
    }

    #[test]
    fn unknown_decision_falls_back_to_manual() {
        let s = build_permission_argv(Some("bypass"), Some("garbage")).unwrap();
        assert!(
            s.contains(r#""decision":"manual""#),
            "garbage decision should fall back to manual: {s}"
        );
    }

    #[test]
    fn returns_valid_json() {
        let s = build_permission_argv(Some("ask_dangerous"), Some("classifier")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&s).expect("must be valid JSON");
        assert_eq!(parsed["mode"], "ask_dangerous");
        assert_eq!(parsed["decision"], "classifier");
    }
}
