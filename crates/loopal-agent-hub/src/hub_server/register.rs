use std::sync::Arc;

use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::protocol::methods;
use loopal_protocol::UiCapabilities;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ClientRole {
    Agent,
    UiClient,
}

pub(super) struct RegisterResult {
    pub(super) request_id: i64,
    pub(super) name: String,
    pub(super) role: ClientRole,
    pub(super) capabilities: UiCapabilities,
    pub(super) lease_id: String,
}

/// Wait for `hub/register` with valid token. Returns agent name + role.
pub(super) async fn wait_for_register(
    conn: &Arc<Connection<Listening>>,
    rx: &mut tokio::sync::mpsc::Receiver<Incoming>,
    expected_token: &str,
) -> anyhow::Result<RegisterResult> {
    loop {
        let Some(msg) = rx.recv().await else {
            anyhow::bail!("connection closed before hub/register");
        };
        if let Incoming::Request { id, method, params } = msg {
            if method != methods::HUB_REGISTER.name {
                let _ = conn
                    .respond_error(
                        id,
                        loopal_ipc::jsonrpc::INVALID_REQUEST,
                        "expected hub/register first",
                    )
                    .await;
                continue;
            }
            let client_token = params["token"].as_str().unwrap_or("");
            if client_token != expected_token {
                let _ = conn
                    .respond_error(id, loopal_ipc::jsonrpc::INVALID_REQUEST, "invalid token")
                    .await;
                anyhow::bail!("invalid token");
            }
            let name = params["name"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("hub/register: missing 'name'"))?
                .to_string();
            let role = parse_role(conn, id, &params).await?;
            validate_principal_name(conn, id, &name, role).await?;
            let capabilities = parse_capabilities(conn, id, &params).await?;
            let lease_id = uuid::Uuid::new_v4().to_string();
            return Ok(RegisterResult {
                request_id: id,
                name,
                role,
                capabilities,
                lease_id,
            });
        }
    }
}

async fn validate_principal_name(
    conn: &Connection<Listening>,
    id: i64,
    name: &str,
    role: ClientRole,
) -> anyhow::Result<()> {
    let reserved = name
        .get(.."meta:".len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("meta:"));
    if role == ClientRole::Agent && reserved {
        let message = "hub/register: agent name uses reserved principal prefix 'meta:'";
        let _ = conn
            .respond_error(id, loopal_ipc::jsonrpc::INVALID_REQUEST, message)
            .await;
        anyhow::bail!(message);
    }
    Ok(())
}

async fn parse_role(
    conn: &Connection<Listening>,
    id: i64,
    params: &serde_json::Value,
) -> anyhow::Result<ClientRole> {
    match params["role"].as_str() {
        Some("ui_client") => Ok(ClientRole::UiClient),
        Some("agent") => Ok(ClientRole::Agent),
        Some(other) => {
            let message = format!("unknown role: {other}");
            let _ = conn
                .respond_error(id, loopal_ipc::jsonrpc::INVALID_REQUEST, &message)
                .await;
            anyhow::bail!(message);
        }
        None => {
            let message = "hub/register: missing 'role' (expected \"agent\" or \"ui_client\")";
            let _ = conn
                .respond_error(id, loopal_ipc::jsonrpc::INVALID_REQUEST, message)
                .await;
            anyhow::bail!("hub/register: missing role");
        }
    }
}

async fn parse_capabilities(
    conn: &Connection<Listening>,
    id: i64,
    params: &serde_json::Value,
) -> anyhow::Result<UiCapabilities> {
    let Some(value) = params.get("capabilities") else {
        return Ok(UiCapabilities::NONE);
    };
    match serde_json::from_value(value.clone()) {
        Ok(value) => Ok(value),
        Err(error) => {
            let message = format!("hub/register: invalid capabilities: {error}");
            let _ = conn
                .respond_error(id, loopal_ipc::jsonrpc::INVALID_REQUEST, &message)
                .await;
            anyhow::bail!(message);
        }
    }
}
