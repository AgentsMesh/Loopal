//! TUI attach mode — observe an existing Hub instance over TCP.
//!
//! Workflow:
//! 1. TCP-connect to the Hub address printed by the first instance.
//! 2. `hub/register` with `role: "ui_client"` and the auth token.
//! 3. Bridge incoming `agent/event` notifications + `view/resync_required`
//!    into the TUI's event/resync channels.
//! 4. Run the TUI. Permission/question dialogs show in-band via broadcast
//!    events; user response goes back via `hub/permission_response` /
//!    `hub/question_response` through `SessionController`.

use std::sync::Arc;

use tokio::net::TcpStream;
use tokio::sync::{Mutex, mpsc};
use tracing::info;

use loopal_agent_hub::{Hub, HubClient};
use loopal_ipc::Connection;
use loopal_ipc::TcpTransport;
use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_protocol::AgentEvent;
use loopal_session::SessionController;

use crate::cli::Cli;

pub async fn run(
    cli: &Cli,
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
    hub_addr: &str,
) -> anyhow::Result<()> {
    let token = cli
        .hub_token
        .clone()
        .ok_or_else(|| anyhow::anyhow!("--attach-hub requires --hub-token"))?;

    info!(hub = %hub_addr, "TUI attach mode: connecting");
    let stream = TcpStream::connect(hub_addr).await?;
    let transport: Arc<dyn Transport> = Arc::new(TcpTransport::new(stream));
    let conn = Arc::new(Connection::new(transport));
    let mut incoming_rx = conn.start();

    let client_name = format!("tui-{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let response = conn
        .send_request(
            methods::HUB_REGISTER.name,
            serde_json::json!({
                "name": client_name,
                "token": token,
                "role": "ui_client",
            }),
        )
        .await
        .map_err(|e| anyhow::anyhow!("hub/register transport error: {e}"))?;
    if response.get("message").is_some() {
        anyhow::bail!("hub/register failed: {response}");
    }
    info!(client = %client_name, "TUI attach: registered as UI client");

    let (event_tx, event_rx) = mpsc::channel::<AgentEvent>(256);
    let (resync_tx, resync_rx) = mpsc::channel::<()>(8);
    tokio::spawn(async move {
        while let Some(msg) = incoming_rx.recv().await {
            match msg {
                Incoming::Notification { method, .. }
                    if method == methods::VIEW_RESYNC_REQUIRED.name =>
                {
                    let _ = resync_tx.try_send(());
                }
                Incoming::Notification { method, params }
                    if method == methods::AGENT_EVENT.name =>
                {
                    if let Ok(event) = serde_json::from_value::<AgentEvent>(params)
                        && event_tx.send(event).await.is_err()
                    {
                        return;
                    }
                }
                _ => {}
            }
        }
    });

    let model = config.settings.model.clone();
    let hub_noop = Arc::new(Mutex::new(Hub::noop()));
    let hub_client = Arc::new(HubClient::new(conn));
    let session_ctrl = SessionController::with_hub(hub_client, hub_noop);

    let display_path = super::abbreviate_home(cwd);
    let mut app = loopal_tui::app::App::new(session_ctrl, cwd.to_path_buf());
    if let Err(e) = app.seed_view_clients().await {
        tracing::warn!(error = %e, "view/snapshot seed failed, continuing with empty view_clients");
    }
    app.push_welcome(&model, &display_path);

    loopal_tui::run_tui(app, event_rx, resync_rx).await
}
