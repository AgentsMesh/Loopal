use std::sync::Arc;

use loopal_provider_api::ContinuationReason;

use super::{continuation_reason_wire, record_text_metrics};
use crate::agent_loop::cancel::TurnCancel;
use crate::agent_loop::turn_context::TurnContext;

#[test]
fn recovery_retry_has_a_stable_wire_reason() {
    assert_eq!(
        continuation_reason_wire(ContinuationReason::RecoveryRetry),
        "recovery_retry"
    );
}

#[test]
fn empty_text_does_not_erase_existing_turn_metrics() {
    let cancel = TurnCancel::new(
        Default::default(),
        Arc::new(tokio::sync::watch::channel(0u64).0),
    );
    let mut turn = TurnContext::new(1, cancel);
    turn.metrics.text_output_len = 7;
    turn.metrics.text_hash = Some(11);

    record_text_metrics(&mut turn, "");

    assert_eq!(turn.metrics.text_output_len, 7);
    assert_eq!(turn.metrics.text_hash, Some(11));
}
