use std::sync::Arc;

use loopal_ipc::Connection;

use super::{CleanupReason, PendingInteraction};
use crate::HubUplink;
use crate::pending_relay::types::{InteractionAudience, PendingQuestionInfo};

fn connection() -> Arc<Connection<loopal_ipc::Listening>> {
    let (_peer, transport) = loopal_ipc::duplex_pair();
    Connection::new(transport).into_listening().0
}

fn remote_question() -> PendingInteraction {
    PendingInteraction::Question {
        id: "logical".into(),
        info: PendingQuestionInfo {
            agent_conn: connection(),
            agent_ipc_id: 1,
            agent_name: "agent".into(),
            interaction_id: "token".into(),
            logical_id: "logical".into(),
            audience: InteractionAudience::RemoteUi {
                target_hub: "other".into(),
                uplink: Arc::new(HubUplink::new(connection(), "hub".into())),
            },
        },
    }
}

#[tokio::test]
async fn remote_cancel_only_exists_for_remote_questions() {
    let cancel = remote_question().remote_cancel().unwrap();
    assert_eq!(cancel.target_hub, "other");
    assert_eq!(cancel.origin_agent, "agent");
    assert_eq!(cancel.interaction_id, "token");
}

#[test]
fn plan_cancellation_reasons_cover_every_cleanup_cause() {
    assert_eq!(
        super::plan_cancellation_reason(CleanupReason::AgentDisconnected),
        "transport"
    );
    assert_eq!(
        super::plan_cancellation_reason(CleanupReason::AgentInterrupted),
        "interrupted"
    );
    assert_eq!(
        super::plan_cancellation_reason(CleanupReason::RequestCancelled),
        "interrupted"
    );
    assert_eq!(
        super::plan_cancellation_reason(CleanupReason::NoCapableUi),
        "unavailable"
    );
    assert_eq!(
        super::plan_cancellation_reason(CleanupReason::TimedOut),
        "timed_out"
    );
}
