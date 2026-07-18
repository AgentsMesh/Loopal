use std::time::Duration;

use serde_json::json;

use crate::support::CliHarness;

/// Cross-process cancellation over the wire: while a turn is blocked on a slow
/// LLM response, an `agent/interrupt` must cancel the in-flight turn in the real
/// agent subprocess — not wait out the (8s) mock delay or report Finished.
#[tokio::test]
async fn agent_cancels_an_in_flight_turn_on_interrupt_over_the_wire() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "cancel_in_flight",
        "calls": [{
            "expect": {"userContains": "hang then cancel"},
            "delayMs": 8000,
            "chunks": [{"type": "text", "text": "unreached"}, {"type": "done"}]
        }]
    }))
    .await;

    h.begin_turn("hang then cancel").await;
    // Let the turn reach the provider and block on the delayed mock response.
    tokio::time::sleep(Duration::from_millis(3000)).await;
    h.interrupt().await;

    let out = h.await_settled(Duration::from_secs(6)).await;
    assert!(
        out.cancelled,
        "interrupt should cancel the in-flight turn well before the 8s mock delay; events: {:?}",
        out.events
    );
    assert!(
        !out.finished,
        "a cancelled turn must not report Finished; events: {:?}",
        out.events
    );
}
