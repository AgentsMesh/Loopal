//! Edge-case tests for HubFrontend: stale interrupt handling and
//! interrupt-then-continue flow.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use loopal_protocol::{Envelope, InterruptSignal, MessageSource};
use loopal_runtime::agent_input::AgentInput;
use loopal_runtime::frontend::traits::AgentFrontend;

use loopal_agent_server::hub_frontend::HubFrontend;
use loopal_agent_server::session_hub::{InputFromClient, SharedSession};

const T: Duration = Duration::from_secs(5);

fn make_session() -> (
    Arc<SharedSession>,
    tokio::sync::mpsc::Sender<InputFromClient>,
    tokio::sync::mpsc::Receiver<InputFromClient>,
    tokio::sync::watch::Receiver<u64>,
) {
    let (input_tx, input_rx) = tokio::sync::mpsc::channel(16);
    let interrupt = InterruptSignal::new();
    let (watch_tx, watch_rx) = tokio::sync::watch::channel(0u64);
    let session = Arc::new(SharedSession {
        session_id: "test-session".into(),
        clients: Mutex::new(Vec::new()),
        input_tx: input_tx.clone(),
        interrupt,
        interrupt_tx: Arc::new(watch_tx),
    });
    (session, input_tx, input_rx, watch_rx)
}

/// Stale interrupt (fired before recv_input) must NOT cause recv_input to
/// return None.  This is the exact bug where the agent loop exited after
/// an interrupt-during-tool because the watch notification was never consumed.
#[tokio::test]
async fn stale_interrupt_does_not_exit_recv_input() {
    let (session, input_tx, input_rx, watch_rx) = make_session();
    let interrupt_tx = session.interrupt_tx.clone();

    // Fire the interrupt BEFORE creating the frontend / calling recv_input.
    // This simulates the interrupt that was already handled by TurnCancel.
    interrupt_tx.send_modify(|v| *v = v.wrapping_add(1));

    let frontend = HubFrontend::new(session, input_rx, None, watch_rx);

    // Spawn recv_input — it should block (stale interrupt consumed), not return None.
    let recv_task = tokio::spawn(async move { frontend.recv_input().await });

    // Give it a moment to ensure it's actually blocking.
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        !recv_task.is_finished(),
        "recv_input should block, not exit"
    );

    // Now send a real message — recv_input should return it.
    let env = Envelope::new(MessageSource::Human, "main", "hello after interrupt");
    input_tx.send(InputFromClient::Message(env)).await.unwrap();

    let result = tokio::time::timeout(T, recv_task).await.unwrap().unwrap();
    assert!(
        matches!(result, Some(AgentInput::Message(_))),
        "should receive the queued message"
    );
}

/// Full interrupt-then-continue cycle: recv_input returns None on a live
/// interrupt, then on the NEXT call it blocks normally (doesn't see the
/// stale notification).
#[tokio::test]
async fn interrupt_then_continue_cycle() {
    let (session, input_tx, input_rx, watch_rx) = make_session();
    let interrupt_tx = session.interrupt_tx.clone();

    let frontend = Arc::new(HubFrontend::new(session, input_rx, None, watch_rx));

    // ── Round 1: live interrupt while recv_input is waiting ──
    let f1 = frontend.clone();
    let recv1 = tokio::spawn(async move { f1.recv_input().await });

    tokio::time::sleep(Duration::from_millis(50)).await;
    interrupt_tx.send_modify(|v| *v = v.wrapping_add(1));

    let result1 = tokio::time::timeout(T, recv1).await.unwrap().unwrap();
    assert!(
        result1.is_none(),
        "round 1: should return None on interrupt"
    );

    // ── Round 2: recv_input must NOT exit due to stale interrupt ──
    let f2 = frontend.clone();
    let recv2 = tokio::spawn(async move { f2.recv_input().await });

    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        !recv2.is_finished(),
        "round 2: recv_input should block, stale interrupt consumed"
    );

    // Send a message — should be delivered.
    let env = Envelope::new(MessageSource::Human, "main", "continue working");
    input_tx.send(InputFromClient::Message(env)).await.unwrap();

    let result2 = tokio::time::timeout(T, recv2).await.unwrap().unwrap();
    assert!(
        matches!(result2, Some(AgentInput::Message(_))),
        "round 2: should receive message"
    );
}

/// Multiple consecutive interrupts: each recv_input call after the first
/// should not be affected by accumulated stale signals.
#[tokio::test]
async fn multiple_stale_interrupts_all_consumed() {
    let (session, input_tx, input_rx, watch_rx) = make_session();
    let interrupt_tx = session.interrupt_tx.clone();

    // Fire three interrupts before creating the frontend.
    for _ in 0..3 {
        interrupt_tx.send_modify(|v| *v = v.wrapping_add(1));
    }

    let frontend = HubFrontend::new(session, input_rx, None, watch_rx);

    let recv_task = tokio::spawn(async move { frontend.recv_input().await });

    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        !recv_task.is_finished(),
        "should block despite 3 stale interrupts"
    );

    let env = Envelope::new(MessageSource::Human, "main", "msg");
    input_tx.send(InputFromClient::Message(env)).await.unwrap();

    let result = tokio::time::timeout(T, recv_task).await.unwrap().unwrap();
    assert!(matches!(result, Some(AgentInput::Message(_))));
}
