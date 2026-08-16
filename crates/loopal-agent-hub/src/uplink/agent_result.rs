use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};
use loopal_protocol::{AgentCompletion, Envelope, MessageSource};
use tokio::sync::Mutex;

use crate::hub::Hub;

pub(super) enum Admission {
    NotAgentResult(Envelope),
    Consumed,
    Deliver {
        envelope: Envelope,
        parent_generation: u64,
    },
}

pub(super) async fn admit(
    hub: &Arc<Mutex<Hub>>,
    connection: &Arc<Connection<Listening>>,
    mut envelope: Envelope,
) -> Admission {
    let MessageSource::AgentResult { child } = &envelope.source else {
        return Admission::NotAgentResult(envelope);
    };
    let child = child.agent.clone();
    let completion = envelope
        .agent_completion
        .take()
        .unwrap_or_else(|| AgentCompletion::goal(Some(envelope.content.text.clone())));
    let redaction_seed = hub.lock().await.final_sink_redaction_seed();
    let (envelope, completion) =
        crate::completion_guard::canonicalize_agent_result(envelope, completion, &redaction_seed);
    let drop_completion = {
        let locked = hub.lock().await;
        !locked.is_active_uplink_connection(connection)
            || locked.should_drop_quarantined_completion(&child, connection)
    };
    if drop_completion {
        tracing::warn!(agent = %child, "dropping completion from stale/quarantined uplink lease");
        return Admission::Consumed;
    }
    if crate::finish::cache_cross_hub_completion_if_spawning(
        hub,
        &child,
        completion.clone(),
        envelope.clone(),
    )
    .await
    {
        return Admission::Consumed;
    }
    let route =
        crate::finish::record_cross_hub_completion_from_uplink(hub, &child, completion, connection)
            .await;
    match route.local_parent_generation() {
        Some(parent_generation) => Admission::Deliver {
            envelope,
            parent_generation,
        },
        None => Admission::Consumed,
    }
}
