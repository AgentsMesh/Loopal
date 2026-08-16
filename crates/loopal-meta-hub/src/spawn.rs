use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::cross_hub::RemoteSpawnOutcome;
use loopal_ipc::protocol::methods;
use loopal_protocol::QualifiedAddress;
use serde_json::Value;
use tokio::sync::Mutex;

use crate::MetaHub;

const FORWARD_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) async fn handle_meta_spawn(
    meta_hub: &Arc<Mutex<MetaHub>>,
    params: Value,
    from_hub: &str,
) -> Result<Value, String> {
    let target_hub = params["target_hub"]
        .as_str()
        .ok_or("missing 'target_hub'")?
        .to_string();
    let mut forwarded = params;
    if let Some(object) = forwarded.as_object_mut() {
        object.remove("target_hub");
    }
    loopal_ipc::cross_hub::validate_forwarded_spawn_payload(&forwarded)?;
    validate_parent(&forwarded, from_hub)?;

    let connection = {
        let meta_hub = meta_hub.lock().await;
        meta_hub
            .registry
            .connection(&target_hub)
            .ok_or_else(|| format!("hub '{target_hub}' not connected"))?
    };
    let outcome = match tokio::time::timeout(
        FORWARD_TIMEOUT,
        connection.send_request(methods::HUB_SPAWN_REMOTE_AGENT.name, forwarded),
    )
    .await
    {
        Ok(Ok(response)) => RemoteSpawnOutcome::Spawned { response },
        Ok(Err(loopal_ipc::RpcError::Remote { message, .. })) => {
            RemoteSpawnOutcome::RejectedBeforeSideEffect {
                message: format!("spawn on '{target_hub}' rejected: {message}"),
            }
        }
        Ok(Err(error)) => RemoteSpawnOutcome::OutcomeUnknown {
            message: format!("spawn on '{target_hub}' transport failed: {error}"),
        },
        Err(_) => RemoteSpawnOutcome::OutcomeUnknown {
            message: format!("spawn on '{target_hub}' timed out"),
        },
    };
    Ok(outcome.into_value())
}

fn validate_parent(params: &Value, from_hub: &str) -> Result<(), String> {
    let value = params["parent"]
        .as_str()
        .expect("forwarded payload validation checked parent");
    let parent = QualifiedAddress::parse(value);
    if !parent.is_remote()
        || parent.agent.trim().is_empty()
        || parent.hub.iter().any(|hub| hub.trim().is_empty())
    {
        return Err("cross-hub spawn parent must be a nonempty remote QualifiedAddress".into());
    }
    if parent.next_hop() != Some(from_hub) {
        return Err(format!(
            "cross-hub spawn parent next hop must match authenticated hub '{from_hub}'"
        ));
    }
    Ok(())
}
