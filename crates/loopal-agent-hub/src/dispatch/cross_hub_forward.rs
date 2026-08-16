//! Caller-side cross-hub spawn forwarding: pre-flight checks
//! (schema, name encoding, uplink) → atomic budget-check + shadow
//! pre-registration → `meta/spawn` IPC → rollback on failure.

use std::sync::Arc;

use serde_json::{Value, json};
use tokio::sync::Mutex;

use loopal_protocol::{AgentEvent, AgentEventPayload};

use crate::authoritative_events::AuthoritativeEventSink;
use crate::hub::Hub;
use crate::types::AgentExecutionRef;
use crate::uplink::HubUplink;
use crate::uplink_requests::SpawnAgentRequestError;

/// Validated cross-hub spawn pre-flight (schema + name encoding + uplink).
/// Budget check and shadow registration happen atomically AFTER this.
struct ForwardPreflight {
    name: String,
    uplink: Arc<HubUplink>,
    hub_name: String,
}

fn check_payload_and_names(params: &Value, from_agent: &str) -> Result<String, String> {
    // Defense-in-depth: cross-hub spawn must not carry filesystem-coupled
    // fields. Reject (don't silently strip) so client-side bugs surface
    // immediately rather than producing surprising behavior on the receiver.
    loopal_ipc::cross_hub::validate_spawn_payload(params)?;

    let name = params["name"]
        .as_str()
        .ok_or("missing 'name' field")?
        .to_string();

    // Reject '/' in agent names: QualifiedAddress encodes hub/agent as
    // slash-joined string, so a name like "foo/bar" produces ambiguous
    // round-trip — receiver parses caller's `hub-a/foo/bar` as
    // hub=["hub-a","foo"], agent="bar". Forbid it at the cross-hub edge.
    if name.contains('/') {
        return Err(format!(
            "agent name '{name}' cannot contain '/' (cross-hub address encoding)"
        ));
    }
    if from_agent.contains('/') {
        return Err(format!(
            "caller agent name '{from_agent}' cannot contain '/' (cross-hub address encoding)"
        ));
    }
    Ok(name)
}

async fn preflight(
    hub: &Arc<Mutex<Hub>>,
    params: &Value,
    requester: &AgentExecutionRef,
) -> Result<ForwardPreflight, String> {
    let name = check_payload_and_names(params, &requester.address.agent)?;
    let h = hub.lock().await;
    let uplink = h
        .uplink
        .clone()
        .ok_or("target_hub specified but no MetaHub uplink")?;
    let hub_name = uplink.hub_name().to_string();
    Ok(ForwardPreflight {
        name,
        uplink,
        hub_name,
    })
}

pub(super) async fn forward_cross_hub_spawn(
    hub: &Arc<Mutex<Hub>>,
    params: Value,
    requester: &AgentExecutionRef,
) -> Result<Value, String> {
    let from_agent = requester.address.agent.as_str();
    let pf = preflight(hub, &params, requester).await?;

    let mut spawn_params = params.clone();
    if let Some(obj) = spawn_params.as_object_mut() {
        // Encode parent so the receiving hub can route completions back to
        // this hub's local caller via MetaHub.
        let parent_addr = loopal_protocol::QualifiedAddress::remote([pf.hub_name], from_agent);
        obj.insert("parent".into(), json!(parent_addr.to_string()));
    }
    loopal_ipc::cross_hub::validate_forwarded_spawn_payload(&spawn_params)?;

    // Completion always travels back from the remote Hub to resolve this
    // shadow's waiters. The shadow separately owns whether that completion is
    // also pushed into the parent Agent's frontend.
    let (_, notify_parent_on_completion) =
        super::spawn_parent_policy::local_parent_policy(&params, from_agent)?;

    let admission = super::cross_hub_spawn_admission::audit_and_register_shadow(
        hub,
        &pf.name,
        &params,
        requester,
        &pf.uplink,
        notify_parent_on_completion,
    )
    .await?;
    let event_sink = admission.event_sink;
    let parent_generation = admission.parent_generation;
    let shadow_generation = admission.shadow_generation;

    // Once the shadow is committed, the remote RPC, event admission, and
    // rollback form one durable coordinator. Cancelling the caller detaches
    // this task instead of leaving an unknown remote spawn without either its
    // SubAgentSpawned observation or a local rollback on definitive failure.
    let coordinator_hub = hub.clone();
    let remote_name = pf.name.clone();
    let coordinator_name = remote_name.clone();
    let parent_name = from_agent.to_string();
    let uplink = pf.uplink.clone();
    let outer_uplink = uplink.clone();
    let destination_hub = params["target_hub"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let outer_destination_hub = destination_hub.clone();
    let model = params["model"].as_str().map(String::from);
    let outer_model = model.clone();
    let outer_event_sink = event_sink.clone();
    let outer_parent_name = parent_name.clone();
    let coordinator = tokio::spawn(async move {
        match uplink.spawn_agent_classified(spawn_params).await {
            Ok(resp) => {
                if !shadow_generation_is_current(
                    &coordinator_hub,
                    &coordinator_name,
                    shadow_generation,
                )
                .await
                {
                    quarantine_and_remove_admission(
                        &coordinator_hub,
                        &coordinator_name,
                        shadow_generation,
                        &uplink,
                    )
                    .await;
                    interrupt_remote_best_effort(uplink, destination_hub, coordinator_name.clone());
                    return Err(format!(
                        "remote agent '{}' spawned after its local shadow generation was replaced",
                        coordinator_name
                    ));
                }
                if !uplink_lease_is_current(&coordinator_hub, &uplink).await {
                    let detail = "spawn response arrived on a superseded MetaHub uplink lease";
                    let resolution = resolve_unknown_outcome(
                        &coordinator_hub,
                        &event_sink,
                        &coordinator_name,
                        shadow_generation,
                        &uplink,
                        &parent_name,
                        parent_generation,
                        model,
                        detail,
                    )
                    .await;
                    interrupt_remote_best_effort(uplink, destination_hub, coordinator_name.clone());
                    return Err(format!(
                        "remote spawn outcome unknown for '{}'; {resolution}: {detail}",
                        coordinator_name
                    ));
                }
                let agent_id = resp["agent_id"].as_str().unwrap_or("unknown").to_string();
                admit_spawn_observation(
                    &event_sink,
                    &coordinator_name,
                    agent_id,
                    model,
                    &parent_name,
                    parent_generation,
                )
                .await
                .map_err(|error| {
                    tracing::error!(
                        agent = %coordinator_name,
                        %error,
                        "cross-hub SubAgentSpawned admission failed; preserving completion shadow"
                    );
                    format!(
                        "remote agent '{}' spawned, but Hub event admission failed: {error}",
                        coordinator_name
                    )
                })?;
                if let Some(cached) = take_shadow_spawn_completion(
                    &coordinator_hub,
                    &coordinator_name,
                    shadow_generation,
                    &uplink,
                )
                .await
                {
                    drain_cached_completion(
                        &coordinator_hub,
                        &coordinator_name,
                        shadow_generation,
                        &uplink,
                        cached,
                    )
                    .await;
                }
                Ok(resp)
            }
            Err(SpawnAgentRequestError::Rejected(error)) => {
                let cached = {
                    let mut h = coordinator_hub.lock().await;
                    let cached = h.take_shadow_spawn_completion(
                        &coordinator_name,
                        shadow_generation,
                        &uplink,
                    );
                    if cached.is_none() {
                        h.registry
                            .unregister_generation_if_current(&coordinator_name, shadow_generation);
                    }
                    cached
                };
                if let Some(cached) = cached {
                    // A typed completion is stronger evidence than a
                    // contradictory rejection. Preserve observable ordering
                    // and converge the waiters on the real terminal result.
                    let _ = admit_spawn_observation(
                        &event_sink,
                        &coordinator_name,
                        "unknown".into(),
                        model,
                        &parent_name,
                        parent_generation,
                    )
                    .await;
                    drain_cached_completion(
                        &coordinator_hub,
                        &coordinator_name,
                        shadow_generation,
                        &uplink,
                        cached,
                    )
                    .await;
                    return Err(format!(
                        "remote spawn for '{}' was rejected after a completion was observed: {error}",
                        coordinator_name
                    ));
                }
                Err(error)
            }
            Err(SpawnAgentRequestError::OutcomeUnknown(error)) => {
                let resolution = resolve_unknown_outcome(
                    &coordinator_hub,
                    &event_sink,
                    &coordinator_name,
                    shadow_generation,
                    &uplink,
                    &parent_name,
                    parent_generation,
                    model,
                    &error,
                )
                .await;
                interrupt_remote_best_effort(uplink, destination_hub, coordinator_name.clone());
                Err(format!(
                    "remote spawn outcome unknown for '{}'; {resolution}: {error}",
                    coordinator_name,
                ))
            }
        }
    });
    match coordinator.await {
        Ok(result) => result,
        Err(error) => {
            hub.lock().await.shutdown_signal.notify_one();
            let detail = format!("spawn coordinator failed: {error}");
            let resolution = resolve_unknown_outcome(
                hub,
                &outer_event_sink,
                &remote_name,
                shadow_generation,
                &outer_uplink,
                &outer_parent_name,
                parent_generation,
                outer_model,
                &detail,
            )
            .await;
            interrupt_remote_best_effort(outer_uplink, outer_destination_hub, remote_name.clone());
            Err(format!(
                "remote agent '{remote_name}' spawn coordinator failed; {resolution}: {error}"
            ))
        }
    }
}

async fn admit_spawn_observation(
    sink: &AuthoritativeEventSink,
    name: &str,
    agent_id: String,
    model: Option<String>,
    parent_name: &str,
    parent_generation: Option<u64>,
) -> Result<(), crate::authoritative_events::AuthoritativeEventQueueClosed> {
    let parent = loopal_protocol::QualifiedAddress::local(parent_name);
    let mut event = AgentEvent::named(
        parent.clone(),
        AgentEventPayload::SubAgentSpawned(loopal_protocol::SubAgentSpawn {
            name: name.to_string(),
            agent_id,
            parent: Some(parent),
            model,
            session_id: None,
        }),
    );
    event.routing_generation = parent_generation;
    sink.prepare(event).deliver().await
}

async fn shadow_generation_is_current(hub: &Arc<Mutex<Hub>>, name: &str, generation: u64) -> bool {
    hub.lock().await.registry.generation(name) == Some(generation)
}

async fn uplink_lease_is_current(hub: &Arc<Mutex<Hub>>, uplink: &Arc<HubUplink>) -> bool {
    hub.lock()
        .await
        .uplink
        .as_ref()
        .is_some_and(|active| Arc::ptr_eq(active, uplink))
}

async fn take_shadow_spawn_completion(
    hub: &Arc<Mutex<Hub>>,
    name: &str,
    generation: u64,
    uplink: &Arc<HubUplink>,
) -> Option<crate::hub::CachedShadowCompletion> {
    hub.lock()
        .await
        .take_shadow_spawn_completion(name, generation, uplink)
}

async fn quarantine_and_remove_admission(
    hub: &Arc<Mutex<Hub>>,
    name: &str,
    generation: u64,
    uplink: &Arc<HubUplink>,
) {
    let mut h = hub.lock().await;
    h.quarantine_shadow_name(name, uplink.clone());
    let _ = h.take_shadow_spawn_completion(name, generation, uplink);
}

async fn drain_cached_completion(
    hub: &Arc<Mutex<Hub>>,
    name: &str,
    generation: u64,
    uplink: &Arc<HubUplink>,
    cached: crate::hub::CachedShadowCompletion,
) {
    let route = crate::finish::record_cross_hub_completion_for_generation(
        hub,
        name,
        generation,
        uplink,
        cached.completion,
    )
    .await;
    if let Some(parent_generation) = route.local_parent_generation()
        && !crate::uplink::reverse_route::deliver_for_generation(
            hub,
            &cached.envelope,
            parent_generation,
        )
        .await
    {
        tracing::warn!(agent = %name, target = %cached.envelope.target,
            "cached remote completion could not be delivered to parent");
    }
}

#[allow(clippy::too_many_arguments)]
async fn resolve_unknown_outcome(
    hub: &Arc<Mutex<Hub>>,
    sink: &AuthoritativeEventSink,
    name: &str,
    generation: u64,
    uplink: &Arc<HubUplink>,
    parent_name: &str,
    parent_generation: Option<u64>,
    model: Option<String>,
    detail: &str,
) -> &'static str {
    if !shadow_generation_is_current(hub, name, generation).await {
        quarantine_and_remove_admission(hub, name, generation, uplink).await;
        return "stale local shadow quarantined";
    }

    let spawned_admission = admit_spawn_observation(
        sink,
        name,
        "unknown".into(),
        model,
        parent_name,
        parent_generation,
    )
    .await;

    let cached = {
        let mut h = hub.lock().await;
        h.take_shadow_spawn_completion(name, generation, uplink)
    };
    let reconciled_cached_completion = cached.is_some();
    if let Some(cached) = cached {
        drain_cached_completion(hub, name, generation, uplink, cached).await;
    } else {
        let completion = loopal_protocol::AgentCompletion::new(
            "remote_spawn_outcome_unknown",
            Some(format!("remote spawn outcome unknown: {detail}")),
        );
        crate::finish::record_cross_hub_completion_for_generation(
            hub, name, generation, uplink, completion,
        )
        .await;
    }

    if spawned_admission.is_err() {
        "shadow terminalized, but SubAgentSpawned admission failed"
    } else if reconciled_cached_completion {
        "cached remote completion reconciled"
    } else {
        "shadow terminalized fail-closed"
    }
}

fn interrupt_remote_best_effort(uplink: Arc<HubUplink>, target_hub: String, name: String) {
    tokio::spawn(async move {
        let result = uplink
            .relay_remote(json!({
                "target_hub": target_hub,
                "operation": "interrupt",
                "payload": {"target": name},
            }))
            .await;
        if let Err(error) = result {
            tracing::warn!(agent = %name, %error, "best-effort remote spawn cleanup failed");
        }
    });
}

#[cfg(test)]
#[path = "cross_hub_spawn_audit_tests.rs"]
mod audit_tests;
#[cfg(test)]
#[path = "cross_hub_forward_reconciliation_tests.rs"]
mod reconciliation_tests;
#[cfg(test)]
#[path = "cross_hub_forward_validation_tests.rs"]
mod validation_tests;

#[cfg(test)]
#[path = "cross_hub_forward_tests.rs"]
mod tests;
