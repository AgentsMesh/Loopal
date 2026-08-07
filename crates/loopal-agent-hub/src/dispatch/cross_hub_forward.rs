//! Caller-side cross-hub spawn forwarding: pre-flight checks
//! (schema, name encoding, uplink) → atomic budget-check + shadow
//! pre-registration → `meta/spawn` IPC → rollback on failure.

use std::sync::Arc;

use serde_json::{Value, json};
use tokio::sync::Mutex;

use loopal_protocol::{AgentEvent, AgentEventPayload};

use crate::authoritative_events::AuthoritativeEventSink;
use crate::hub::Hub;
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
    from_agent: &str,
) -> Result<ForwardPreflight, String> {
    let name = check_payload_and_names(params, from_agent)?;
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
    from_agent: &str,
) -> Result<Value, String> {
    let pf = preflight(hub, &params, from_agent).await?;

    let mut spawn_params = params.clone();
    if let Some(obj) = spawn_params.as_object_mut() {
        // Encode parent so the receiving hub can route completions back to
        // this hub's local caller via MetaHub.
        let parent_addr = loopal_protocol::QualifiedAddress::remote([pf.hub_name], from_agent);
        obj.insert("parent".into(), json!(parent_addr.to_string()));
    }

    // Completion always travels back from the remote Hub to resolve this
    // shadow's waiters. The shadow separately owns whether that completion is
    // also pushed into the parent Agent's frontend.
    let (_, notify_parent_on_completion) =
        super::spawn_parent_policy::local_parent_policy(&params, from_agent)?;

    // Atomic budget check + shadow registration: holding the same lock
    // across both prevents two concurrent cross-hub spawns from each
    // observing budget=N-1 and overshooting. Pre-registering before the
    // IPC also closes the race where a fast-completing remote child's
    // envelope arrives before the spawn response — `emit_agent_finished`
    // would otherwise return None (no entry for the child), and the
    // parent's local completion_tx would never receive the agent-result
    // envelope. With the shadow present, emit_agent_finished can read the
    // shadow's `info.parent` and route the envelope to the local parent.
    let (event_sink, parent_generation, shadow_generation) = {
        let mut h = hub.lock().await;
        if !h
            .uplink
            .as_ref()
            .is_some_and(|active| Arc::ptr_eq(active, &pf.uplink))
        {
            return Err("MetaHub uplink changed during remote spawn admission".into());
        }
        if h.shadow_name_is_quarantined(&pf.name, &pf.uplink) {
            return Err(format!(
                "remote agent name '{}' is quarantined until the MetaHub uplink reconnects",
                pf.name
            ));
        }
        let sub_count = h.registry.sub_agent_count();
        if sub_count >= h.max_total_agents as usize {
            return Err(format!(
                "Spawn budget exhausted ({sub_count}/{} sub-agents). \
                 Complete the task with your own tools.",
                h.max_total_agents
            ));
        }
        h.registry.register_shadow_with_parent_policy(
            &pf.name,
            loopal_protocol::QualifiedAddress::local(from_agent),
            notify_parent_on_completion,
        )?;
        let shadow_generation = h
            .registry
            .generation(&pf.name)
            .expect("newly registered shadow must own a generation");
        h.install_shadow_spawn_admission(&pf.name, shadow_generation, pf.uplink.clone());
        (
            AuthoritativeEventSink::from_hub(&h),
            h.registry.generation(from_agent),
            shadow_generation,
        )
    };

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
mod tests {
    use std::time::Duration;

    use loopal_ipc::Connection;
    use loopal_ipc::connection::Incoming;
    use loopal_ipc::protocol::methods;
    use loopal_protocol::{AgentEvent, AgentEventPayload};
    use tokio::sync::{Mutex, mpsc};

    use super::*;

    async fn hub_with_uplink(
        event_tx: mpsc::Sender<AgentEvent>,
    ) -> (
        Arc<Mutex<Hub>>,
        Arc<Connection<loopal_ipc::Listening>>,
        mpsc::Receiver<Incoming>,
    ) {
        let (hub_transport, meta_transport) = loopal_ipc::duplex_pair();
        let (hub_connection, _hub_rx) = Connection::new(hub_transport).into_listening();
        let (meta_connection, meta_rx) = Connection::new(meta_transport).into_listening();
        let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
        hub.lock().await.uplink = Some(Arc::new(HubUplink::new(hub_connection, "origin".into())));
        (hub, meta_connection, meta_rx)
    }

    fn respond_to_spawn(
        meta_connection: Arc<Connection<loopal_ipc::Listening>>,
        mut meta_rx: mpsc::Receiver<Incoming>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let Incoming::Request { id, method, .. } = meta_rx.recv().await.unwrap() else {
                panic!("expected meta/spawn request");
            };
            assert_eq!(method, methods::META_SPAWN.name);
            meta_connection
                .respond(id, json!({"agent_id": "remote-id"}))
                .await
                .unwrap();
        })
    }

    #[tokio::test]
    async fn full_queue_backpressures_cross_hub_spawn_without_holding_hub_lock() {
        let (event_tx, mut event_rx) = mpsc::channel(1);
        event_tx
            .send(AgentEvent::root(AgentEventPayload::Running))
            .await
            .unwrap();
        let (hub, meta_connection, meta_rx) = hub_with_uplink(event_tx).await;
        let responder = respond_to_spawn(meta_connection, meta_rx);
        let spawn = tokio::spawn({
            let hub = hub.clone();
            async move {
                forward_cross_hub_spawn(
                    &hub,
                    json!({
                        "name": "remote-worker",
                        "prompt": "work",
                        "target_hub": "destination",
                    }),
                    "main",
                )
                .await
            }
        });
        responder.await.unwrap();
        tokio::task::yield_now().await;
        assert!(
            !spawn.is_finished(),
            "remote spawn must wait for SubAgentSpawned queue capacity"
        );
        assert!(
            hub.lock()
                .await
                .registry
                .agent_info("remote-worker")
                .is_some()
        );
        let guard = tokio::time::timeout(Duration::from_millis(100), hub.lock())
            .await
            .expect("cross-hub event backpressure must not hold the Hub lock");
        drop(guard);

        assert!(matches!(
            event_rx.recv().await.unwrap().payload,
            AgentEventPayload::Running
        ));
        let event = event_rx.recv().await.unwrap();
        assert!(matches!(
            event.payload,
            AgentEventPayload::SubAgentSpawned(ref spawned)
                if spawned.name == "remote-worker" && spawned.agent_id == "remote-id"
        ));
        assert_eq!(spawn.await.unwrap().unwrap()["agent_id"], "remote-id");
    }

    #[tokio::test]
    async fn closed_queue_reports_failure_but_preserves_remote_completion_shadow() {
        let (event_tx, event_rx) = mpsc::channel(1);
        drop(event_rx);
        let (hub, meta_connection, meta_rx) = hub_with_uplink(event_tx).await;
        let shutdown = hub.lock().await.shutdown_signal.clone();
        let responder = respond_to_spawn(meta_connection, meta_rx);

        let error = forward_cross_hub_spawn(
            &hub,
            json!({
                "name": "remote-worker",
                "prompt": "work",
                "target_hub": "destination",
            }),
            "main",
        )
        .await
        .unwrap_err();
        responder.await.unwrap();
        assert!(error.contains("authoritative Hub event queue closed"));
        assert!(
            hub.lock()
                .await
                .registry
                .agent_info("remote-worker")
                .is_some(),
            "remote child exists, so its shadow must remain for late completion routing"
        );
        tokio::time::timeout(Duration::from_millis(100), shutdown.notified())
            .await
            .expect("closed authoritative queue must invalidate the Hub");
    }

    #[tokio::test]
    async fn remote_spawn_timeout_terminalizes_in_order_and_quarantines_same_lease_name() {
        let (event_tx, mut event_rx) = mpsc::channel(8);
        let (hub, meta_connection, mut meta_rx) = hub_with_uplink(event_tx).await;
        let old_uplink = hub.lock().await.uplink.clone().unwrap();
        let remote = tokio::spawn(async move {
            let Incoming::Request { method, .. } = meta_rx.recv().await.unwrap() else {
                panic!("expected meta/spawn request");
            };
            assert_eq!(method, methods::META_SPAWN.name);
            tokio::time::sleep(Duration::from_millis(150)).await;
            drop(meta_connection);
        });

        let error = forward_cross_hub_spawn(
            &hub,
            json!({
                "name": "unknown-outcome-worker",
                "prompt": "work",
                "target_hub": "destination",
            }),
            "main",
        )
        .await
        .unwrap_err();
        assert!(error.contains("outcome unknown"));
        let spawned = event_rx.recv().await.unwrap();
        let terminal_error = event_rx.recv().await.unwrap();
        let finished = event_rx.recv().await.unwrap();
        assert!(matches!(
            spawned.payload,
            AgentEventPayload::SubAgentSpawned(ref event)
                if event.name == "unknown-outcome-worker" && event.agent_id == "unknown"
        ));
        assert!(matches!(
            terminal_error.payload,
            AgentEventPayload::Error { ref message }
                if message.contains("remote spawn outcome unknown")
        ));
        assert!(matches!(finished.payload, AgentEventPayload::Finished));
        assert_eq!(
            hub.lock()
                .await
                .registry
                .completion("unknown-outcome-worker")
                .map(|completion| completion.reason.as_str()),
            Some("remote_spawn_outcome_unknown")
        );

        let quarantined = forward_cross_hub_spawn(
            &hub,
            json!({
                "name": "unknown-outcome-worker",
                "prompt": "must not reuse",
                "target_hub": "destination",
            }),
            "main",
        )
        .await
        .unwrap_err();
        assert!(quarantined.contains("quarantined"));

        // Even if another registration path reuses the bare name, a late
        // completion on the indeterminate lease cannot finish it.
        let replacement_generation = {
            let mut h = hub.lock().await;
            h.registry
                .register_shadow(
                    "unknown-outcome-worker",
                    loopal_protocol::QualifiedAddress::local("replacement-parent"),
                )
                .unwrap();
            h.registry.generation("unknown-outcome-worker").unwrap()
        };
        assert!(matches!(
            crate::finish::record_cross_hub_completion_from_uplink(
                &hub,
                "unknown-outcome-worker",
                loopal_protocol::AgentCompletion::goal(Some("late old result".into())),
                old_uplink.connection(),
            )
            .await,
            crate::finish::CrossHubCompletionRoute::Consumed
        ));
        assert_eq!(
            hub.lock()
                .await
                .registry
                .generation("unknown-outcome-worker"),
            Some(replacement_generation)
        );
        assert!(
            hub.lock()
                .await
                .registry
                .completion("unknown-outcome-worker")
                .is_none()
        );

        let (new_hub_transport, _new_meta_transport) = loopal_ipc::duplex_pair();
        let (new_connection, _new_rx) = Connection::new(new_hub_transport).into_listening();
        let new_uplink = Arc::new(HubUplink::new(new_connection, "origin".into()));
        {
            let mut h = hub.lock().await;
            h.uplink = Some(new_uplink.clone());
            assert!(
                !h.shadow_name_is_quarantined("unknown-outcome-worker", &new_uplink),
                "a new authenticated uplink lease releases the conservative name quarantine"
            );
        }
        remote.await.unwrap();
    }

    #[tokio::test]
    async fn failed_remote_rpc_cannot_rollback_a_same_name_new_generation() {
        let (event_tx, _event_rx) = mpsc::channel(8);
        let (hub, meta_connection, mut meta_rx) = hub_with_uplink(event_tx).await;
        let (request_seen_tx, request_seen_rx) = tokio::sync::oneshot::channel();
        let (respond_tx, respond_rx) = tokio::sync::oneshot::channel();
        let responder = tokio::spawn(async move {
            let Incoming::Request { id, method, .. } = meta_rx.recv().await.unwrap() else {
                panic!("expected meta/spawn request");
            };
            assert_eq!(method, methods::META_SPAWN.name);
            request_seen_tx.send(()).unwrap();
            respond_rx.await.unwrap();
            meta_connection
                .respond(id, json!({"message": "remote rejected"}))
                .await
                .unwrap();
        });
        let spawn = tokio::spawn({
            let hub = hub.clone();
            async move {
                forward_cross_hub_spawn(
                    &hub,
                    json!({
                        "name": "remote-worker",
                        "prompt": "work",
                        "target_hub": "destination",
                    }),
                    "main",
                )
                .await
            }
        });
        request_seen_rx.await.unwrap();

        let replacement_generation = {
            let mut hub = hub.lock().await;
            hub.registry.unregister_connection("remote-worker");
            hub.registry
                .register_shadow(
                    "remote-worker",
                    loopal_protocol::QualifiedAddress::local("replacement-parent"),
                )
                .unwrap();
            hub.registry.generation("remote-worker").unwrap()
        };
        respond_tx.send(()).unwrap();
        assert!(spawn.await.unwrap().is_err());
        responder.await.unwrap();

        let hub = hub.lock().await;
        assert_eq!(
            hub.registry.generation("remote-worker"),
            Some(replacement_generation)
        );
        assert_eq!(
            hub.registry
                .agent_info("remote-worker")
                .and_then(|info| info.parent.as_ref())
                .map(ToString::to_string)
                .as_deref(),
            Some("replacement-parent")
        );
    }

    #[tokio::test]
    async fn spawn_response_on_superseded_uplink_terminalizes_shadow_fail_closed() {
        let (event_tx, mut event_rx) = mpsc::channel(8);
        let (hub, meta_connection, mut meta_rx) = hub_with_uplink(event_tx).await;
        let (request_seen_tx, request_seen_rx) = tokio::sync::oneshot::channel();
        let (respond_tx, respond_rx) = tokio::sync::oneshot::channel();
        let responder = tokio::spawn(async move {
            let Incoming::Request { id, method, .. } = meta_rx.recv().await.unwrap() else {
                panic!("expected meta/spawn request");
            };
            assert_eq!(method, methods::META_SPAWN.name);
            request_seen_tx.send(()).unwrap();
            respond_rx.await.unwrap();
            meta_connection
                .respond(id, json!({"agent_id": "remote-id-on-old-lease"}))
                .await
                .unwrap();
        });
        let spawn = tokio::spawn({
            let hub = hub.clone();
            async move {
                forward_cross_hub_spawn(
                    &hub,
                    json!({
                        "name": "lease-race-worker",
                        "prompt": "work",
                        "target_hub": "destination",
                    }),
                    "main",
                )
                .await
            }
        });
        request_seen_rx.await.unwrap();

        let (replacement_transport, _replacement_peer) = loopal_ipc::duplex_pair();
        let (replacement_connection, _replacement_rx) =
            Connection::new(replacement_transport).into_listening();
        hub.lock().await.uplink = Some(Arc::new(HubUplink::new(
            replacement_connection,
            "origin".into(),
        )));
        respond_tx.send(()).unwrap();

        let error = spawn.await.unwrap().unwrap_err();
        assert!(error.contains("superseded MetaHub uplink lease"));
        responder.await.unwrap();
        assert!(matches!(
            event_rx.recv().await.unwrap().payload,
            AgentEventPayload::SubAgentSpawned(ref event)
                if event.name == "lease-race-worker" && event.agent_id == "unknown"
        ));
        assert!(matches!(
            event_rx.recv().await.unwrap().payload,
            AgentEventPayload::Error { .. }
        ));
        assert!(matches!(
            event_rx.recv().await.unwrap().payload,
            AgentEventPayload::Finished
        ));
        assert_eq!(
            hub.lock()
                .await
                .registry
                .completion("lease-race-worker")
                .map(|completion| completion.reason.as_str()),
            Some("remote_spawn_outcome_unknown")
        );
    }

    #[tokio::test]
    async fn remote_completion_before_spawn_response_is_cached_without_blocking_and_drained_in_order()
     {
        let (event_tx, mut event_rx) = mpsc::channel(8);
        let (hub, meta_connection, mut meta_rx) = hub_with_uplink(event_tx).await;
        let completion_lease = hub.lock().await.uplink.clone().unwrap();
        let (request_seen_tx, request_seen_rx) = tokio::sync::oneshot::channel();
        let (respond_tx, respond_rx) = tokio::sync::oneshot::channel();
        let responder = tokio::spawn(async move {
            let Incoming::Request { id, method, .. } = meta_rx.recv().await.unwrap() else {
                panic!("expected meta/spawn request");
            };
            assert_eq!(method, methods::META_SPAWN.name);
            request_seen_tx.send(()).unwrap();
            respond_rx.await.unwrap();
            meta_connection
                .respond(id, json!({"agent_id": "instant-remote-id"}))
                .await
                .unwrap();
        });
        let spawn = tokio::spawn({
            let hub = hub.clone();
            async move {
                forward_cross_hub_spawn(
                    &hub,
                    json!({
                        "name": "instant-remote",
                        "prompt": "work",
                        "target_hub": "destination",
                    }),
                    "main",
                )
                .await
            }
        });
        request_seen_rx.await.unwrap();
        let typed_completion =
            loopal_protocol::AgentCompletion::new("error", Some("failed immediately".into()));
        let envelope = loopal_protocol::Envelope::new(
            loopal_protocol::MessageSource::AgentResult {
                child: loopal_protocol::QualifiedAddress::local("instant-remote"),
            },
            loopal_protocol::QualifiedAddress::local("main"),
            "failed immediately",
        )
        .with_agent_completion(typed_completion.clone());
        let cached = tokio::time::timeout(
            Duration::from_millis(100),
            crate::finish::cache_cross_hub_completion_if_spawning(
                &hub,
                "instant-remote",
                typed_completion,
                envelope,
            ),
        )
        .await
        .expect("reverse completion admission must not wait for spawn RPC response");
        assert!(cached);

        respond_tx.send(()).unwrap();
        assert_eq!(
            spawn.await.unwrap().unwrap()["agent_id"],
            "instant-remote-id"
        );
        responder.await.unwrap();

        let spawned = event_rx.recv().await.unwrap();
        assert!(matches!(
            spawned.payload,
            AgentEventPayload::SubAgentSpawned(ref event)
                if event.name == "instant-remote"
        ));
        let error = event_rx.recv().await.unwrap();
        assert!(matches!(error.payload, AgentEventPayload::Error { .. }));
        let finished = event_rx.recv().await.unwrap();
        assert!(matches!(finished.payload, AgentEventPayload::Finished));

        let reuse_error = forward_cross_hub_spawn(
            &hub,
            json!({
                "name": "instant-remote",
                "prompt": "must not reuse on same lease",
                "target_hub": "destination",
            }),
            "main",
        )
        .await
        .unwrap_err();
        assert!(reuse_error.contains("quarantined"));

        // A non-cross-hub registration path cannot make an old duplicate
        // completion authoritative for the replacement generation either.
        let replacement_generation = {
            let mut h = hub.lock().await;
            h.registry
                .register_shadow(
                    "instant-remote",
                    loopal_protocol::QualifiedAddress::local("replacement-parent"),
                )
                .unwrap();
            h.registry.generation("instant-remote").unwrap()
        };
        assert!(matches!(
            crate::finish::record_cross_hub_completion_from_uplink(
                &hub,
                "instant-remote",
                loopal_protocol::AgentCompletion::goal(Some("late duplicate".into())),
                completion_lease.connection(),
            )
            .await,
            crate::finish::CrossHubCompletionRoute::Consumed
        ));
        let h = hub.lock().await;
        assert_eq!(
            h.registry.generation("instant-remote"),
            Some(replacement_generation)
        );
        assert!(h.registry.completion("instant-remote").is_none());
    }
}
