use std::sync::Arc;

use loopal_ipc::Connection;
use loopal_ipc::connection::Incoming;
use loopal_protocol::{AgentCompletion, AgentEvent, Envelope, MessageSource, QualifiedAddress};
use tokio::sync::{Mutex, mpsc, oneshot};

use super::tests::{hub_with_uplink, signed_spawn};
use super::{drain_cached_completion, forward_cross_hub_spawn, resolve_unknown_outcome};
use crate::authoritative_events::AuthoritativeEventSink;
use crate::hub::CachedShadowCompletion;
use crate::{Hub, HubUplink};

fn cached(child: &str, target: &str) -> CachedShadowCompletion {
    let completion = AgentCompletion::new("error", Some("completed early".into()));
    let envelope = Envelope::new(
        MessageSource::AgentResult {
            child: QualifiedAddress::local(child),
        },
        QualifiedAddress::local(target),
        "completed early",
    )
    .with_agent_completion(completion.clone());
    CachedShadowCompletion {
        completion,
        envelope,
    }
}

include!("cross_hub_forward_reconciliation_tests/outcomes.rs");
include!("cross_hub_forward_reconciliation_tests/routes.rs");
