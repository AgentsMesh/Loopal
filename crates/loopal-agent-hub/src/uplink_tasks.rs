use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc};

use loopal_ipc::connection::Incoming;

use crate::{Hub, HubUplink};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);
#[cfg(not(test))]
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(test)]
const HEARTBEAT_TIMEOUT: Duration = Duration::from_millis(100);

pub(crate) fn start(hub: Arc<Mutex<Hub>>, uplink: Arc<HubUplink>, rx: mpsc::Receiver<Incoming>) {
    let reverse_hub = hub.clone();
    let reverse_uplink = uplink.clone();
    tokio::spawn(async move {
        crate::uplink::handle_reverse_requests(
            reverse_hub.clone(),
            reverse_uplink.connection().clone(),
            rx,
            reverse_uplink.hub_name().to_string(),
        )
        .await;
        cleanup(&reverse_hub, &reverse_uplink).await;
    });
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(HEARTBEAT_INTERVAL);
        interval.tick().await;
        loop {
            interval.tick().await;
            if !uplink.connection().is_connected() {
                break;
            }
            let count = hub.lock().await.registry.managed_agent_count();
            if heartbeat(&uplink, count).await.is_err() {
                cleanup(&hub, &uplink).await;
                break;
            }
        }
    });
}

pub(crate) async fn heartbeat(uplink: &HubUplink, count: usize) -> Result<(), String> {
    tokio::time::timeout(HEARTBEAT_TIMEOUT, uplink.heartbeat(count))
        .await
        .map_err(|_| "meta/heartbeat timed out".to_string())?
}

pub(crate) async fn cleanup(hub: &Arc<Mutex<Hub>>, uplink: &Arc<HubUplink>) {
    let mut locked = hub.lock().await;
    if locked
        .uplink
        .as_ref()
        .is_some_and(|active| Arc::ptr_eq(active, uplink))
    {
        locked.uplink = None;
    }
    drop(locked);
    uplink.connection().close().await;
}
