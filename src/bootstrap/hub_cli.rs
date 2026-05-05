use std::sync::Arc;

use anyhow::{Context as _, Result};
use tokio::net::TcpStream;
use tracing::info;

use loopal_agent_hub::HubClient;
use loopal_ipc::Connection;
use loopal_ipc::TcpTransport;
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;

use super::{discovery, token_channel};

pub fn run_list_hubs() {
    let records = discovery::list_records();
    if records.is_empty() {
        eprintln!("No running hubs for the current user.");
        return;
    }
    let headers = ["PID", "TCP_ADDR", "STARTED", "CWD"];
    let rows: Vec<[String; 4]> = records
        .into_iter()
        .map(|r| [r.pid.to_string(), r.tcp_addr, r.started_at, r.cwd])
        .collect();
    print_columns(&headers, &rows);
}

fn print_columns(headers: &[&str; 4], rows: &[[String; 4]]) {
    let mut widths = [0usize; 4];
    for (i, h) in headers.iter().enumerate() {
        widths[i] = h.len();
    }
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.len());
        }
    }
    let mut header_line = String::new();
    for (i, h) in headers.iter().enumerate() {
        if i > 0 {
            header_line.push_str("  ");
        }
        if i == headers.len() - 1 {
            header_line.push_str(h);
        } else {
            header_line.push_str(&format!("{:<w$}", h, w = widths[i]));
        }
    }
    println!("{header_line}");
    for row in rows {
        let mut line = String::new();
        for (i, cell) in row.iter().enumerate() {
            if i > 0 {
                line.push_str("  ");
            }
            if i == row.len() - 1 {
                line.push_str(cell);
            } else {
                line.push_str(&format!("{:<w$}", cell, w = widths[i]));
            }
        }
        println!("{line}");
    }
}

pub async fn resolve_pid(pid: u32) -> Result<(String, String, String)> {
    if !discovery::is_alive(pid) {
        // Stale record left by a crashed Hub. Surface a precise error
        // instead of letting the user see a confusing socket-connect
        // failure downstream.
        anyhow::bail!("hub pid {pid} is not running (record is stale)");
    }
    let record = discovery::read_record(pid)
        .with_context(|| format!("no discovery record for pid {pid}"))?;
    let token = token_channel::fetch_token(pid)
        .await
        .with_context(|| format!("token handoff failed for pid {pid}"))?;
    Ok((record.tcp_addr, token, record.root_session_id))
}

pub async fn run_attach_pid(
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
    pid: u32,
) -> Result<()> {
    let (addr, token, root_session_id) = resolve_pid(pid).await?;
    info!(pid, addr = %addr, "attaching to hub via discovery record");
    let _ =
        super::attach_mode::run_with_addr(cwd, config, &addr, &token, Some(&root_session_id), None)
            .await?;
    Ok(())
}

pub async fn run_kill_hub(pid: u32) -> Result<()> {
    let (addr, token, _) = resolve_pid(pid).await?;
    info!(pid, addr = %addr, "sending hub/shutdown");
    let stream = TcpStream::connect(&addr)
        .await
        .with_context(|| format!("connect {addr}"))?;
    let transport: Arc<dyn Transport> = Arc::new(TcpTransport::new(stream));
    let conn = Arc::new(Connection::new(transport));
    let _rx = conn.start();
    let client_name = format!("kill-{}", &uuid::Uuid::new_v4().to_string()[..8]);
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
        .map_err(|e| anyhow::anyhow!("hub/register failed: {e}"))?;
    if response.get("message").is_some() {
        anyhow::bail!("hub/register rejected: {response}");
    }
    let hub_client = HubClient::new(conn);
    hub_client.shutdown_hub().await;
    eprintln!("hub/shutdown sent to pid {pid}.");
    Ok(())
}
