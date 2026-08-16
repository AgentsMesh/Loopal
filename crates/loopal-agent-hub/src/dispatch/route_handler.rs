use std::sync::Arc;

use loopal_protocol::Envelope;
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::hub::Hub;
use crate::request_principal::HubRequestPrincipal;
use crate::routing;

pub(super) async fn handle_route(
    hub: &Arc<Mutex<Hub>>,
    params: Value,
    principal: &HubRequestPrincipal,
) -> Result<Value, String> {
    let mut envelope: Envelope =
        serde_json::from_value(params).map_err(|e| format!("invalid envelope: {e}"))?;
    envelope.source = match principal {
        HubRequestPrincipal::Ui(_) => loopal_protocol::MessageSource::Human,
        HubRequestPrincipal::Agent(agent) => {
            loopal_protocol::MessageSource::Agent(agent.execution.address.clone())
        }
        HubRequestPrincipal::TrustedMetaHub(_) => {
            return Err("MetaHub reverse transport cannot call hub/route".into());
        }
        HubRequestPrincipal::Internal => envelope.source,
    };
    let cwd = {
        let locked = hub.lock().await;
        locked
            .spawn_registry
            .cwd_of(&envelope.target.agent)
            .unwrap_or_else(|| locked.default_cwd.clone())
    };
    super::skill_routing::expand_human_skill(&mut envelope, &cwd);
    if envelope.target.is_remote() {
        return route_via_uplink(hub, &envelope).await;
    }
    let target = {
        let locked = hub.lock().await;
        locked
            .registry
            .get_agent_connection(&envelope.target.agent)
            .map(|connection| {
                let observation =
                    routing::RouteObservation::from_hub(&locked, &envelope.target.agent);
                (connection, observation)
            })
    };
    match target {
        Some((connection, observation)) => {
            routing::route_to_agent(&connection, &envelope, &observation).await?;
            Ok(json!({"ok": true}))
        }
        None => route_via_uplink(hub, &envelope).await,
    }
}

async fn route_via_uplink(hub: &Arc<Mutex<Hub>>, envelope: &Envelope) -> Result<Value, String> {
    let uplink = hub.lock().await.uplink.clone();
    match uplink {
        Some(uplink) => {
            uplink.route(envelope).await?;
            Ok(json!({"ok": true}))
        }
        None => Err(format!(
            "agent '{}' not found locally and no MetaHub uplink configured",
            envelope.target
        )),
    }
}
