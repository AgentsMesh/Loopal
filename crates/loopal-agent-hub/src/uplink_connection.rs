use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use loopal_ipc::tcp::TcpTransport;

use crate::{Hub, HubUplink};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(not(test))]
const META_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(test)]
const META_REQUEST_TIMEOUT: Duration = Duration::from_millis(100);
const UNREGISTER_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Deserialize)]
pub struct JoinMetaHubParams {
    pub address: String,
    pub token: String,
    pub hub_name: String,
}

pub async fn connect(
    hub: &Arc<Mutex<Hub>>,
    address: &str,
    token: &str,
    hub_name: &str,
) -> Result<Value, String> {
    validate(address, token, hub_name)?;
    let stale = {
        let mut locked = hub.lock().await;
        match locked.uplink.as_ref() {
            Some(active) if active.connection().is_connected() => {
                return Err("Hub is already connected to a MetaHub".into());
            }
            Some(_) => locked.uplink.take(),
            None => None,
        }
    };
    if let Some(stale) = stale {
        crate::pending_relay::cleanup_pending_for_uplink(hub, &stale).await;
        stale.connection().close().await;
    }
    let stream = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(address))
        .await
        .map_err(|_| format!("connect to MetaHub {address} timed out"))?
        .map_err(|error| format!("connect to MetaHub {address}: {error}"))?;
    let transport: Arc<dyn loopal_ipc::transport::Transport> = Arc::new(TcpTransport::new(stream));
    let (conn, rx) = Connection::new(transport).into_listening();
    let registration = tokio::time::timeout(
        META_REQUEST_TIMEOUT,
        conn.send_request(
            methods::META_REGISTER.name,
            json!({"name": hub_name, "token": token, "capabilities": ["desktop"]}),
        ),
    )
    .await;
    let response = match registration {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => {
            conn.close().await;
            return Err(format!("meta/register failed: {error}"));
        }
        Err(_) => {
            conn.close().await;
            return Err("meta/register timed out".into());
        }
    };
    if response.get("ok").and_then(Value::as_bool) != Some(true) {
        conn.close().await;
        return Err(format!("MetaHub rejected registration: {response}"));
    }

    let uplink = Arc::new(HubUplink::with_address(
        conn.clone(),
        hub_name.to_string(),
        address.to_string(),
    ));
    let collision = {
        let mut locked = hub.lock().await;
        if locked.uplink.is_none() {
            locked.uplink = Some(uplink.clone());
            false
        } else {
            true
        }
    };
    if collision {
        conn.close().await;
        return Err("Hub connected to another MetaHub while joining".into());
    }
    let agent_count = hub.lock().await.registry.managed_agent_count();
    if let Err(error) = crate::uplink_tasks::heartbeat(&uplink, agent_count).await {
        crate::uplink_tasks::cleanup(hub, &uplink).await;
        return Err(error);
    }
    crate::uplink_tasks::start(hub.clone(), uplink, rx);
    Ok(json!({"connected": true, "hub_name": hub_name, "address": address}))
}

pub async fn disconnect(hub: &Arc<Mutex<Hub>>) -> Result<Value, String> {
    let uplink = hub.lock().await.uplink.take();
    if let Some(uplink) = uplink {
        crate::pending_relay::cleanup_pending_for_uplink(hub, &uplink).await;
        let unregister = uplink
            .connection()
            .send_request(methods::META_UNREGISTER.name, json!({}));
        let _ = tokio::time::timeout(UNREGISTER_TIMEOUT, unregister).await;
        uplink.connection().close().await;
    }
    Ok(json!({"connected": false}))
}

#[cfg(test)]
#[path = "uplink_connection_tests.rs"]
mod tests;

fn validate(address: &str, token: &str, hub_name: &str) -> Result<(), String> {
    if address.trim().is_empty() || address.len() > 512 {
        return Err("MetaHub address must be 1..512 characters".into());
    }
    if token.is_empty() || token.len() > 4096 {
        return Err("MetaHub token must be 1..4096 characters".into());
    }
    if hub_name.is_empty() || hub_name.len() > 128 || hub_name.contains('/') {
        return Err("MetaHub hub name must be 1..128 characters without '/'".into());
    }
    Ok(())
}
