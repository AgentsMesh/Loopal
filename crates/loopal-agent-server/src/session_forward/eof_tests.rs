use std::sync::Arc;

use loopal_ipc::connection::Incoming;
use loopal_protocol::InterruptSignal;
use loopal_runtime::agent_input::AgentInput;
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;

use super::forwarding_test_support::{connection, pending_handle};
use super::{ForwardResult, forward_loop};
use crate::session_hub::SharedSession;
use crate::session_start::SessionHandle;

fn session(
    id: &str,
) -> (
    Arc<SharedSession>,
    watch::Receiver<u64>,
    mpsc::Receiver<Incoming>,
) {
    let (incoming_tx, incoming_rx) = mpsc::channel(1);
    drop(incoming_tx);
    let (input_tx, _input_rx) = mpsc::channel::<AgentInput>(1);
    let (interrupt_tx, interrupt_rx) = watch::channel(0u64);
    (
        Arc::new(SharedSession::new(
            id.into(),
            input_tx,
            InterruptSignal::new(),
            Arc::new(interrupt_tx),
        )),
        interrupt_rx,
        incoming_rx,
    )
}

fn handle(
    session: Arc<SharedSession>,
    agent_task: tokio::task::JoinHandle<Option<loopal_error::AgentOutput>>,
) -> SessionHandle {
    SessionHandle {
        session_id: session.session_id.clone(),
        session,
        agent_task,
        lifecycle: loopal_runtime::LifecycleMode::Persistent,
        shutdown: CancellationToken::new(),
        redaction_seed: loopal_output_guard::FinalSinkRedactionSeed::new(),
        completion_result_limit: loopal_output_guard::MAX_AGENT_COMPLETION_RESULT_BYTES,
    }
}

#[tokio::test]
async fn resignals_and_aborts_an_unresponsive_agent() {
    let server = connection();
    let (session, mut interrupt_rx, mut incoming_rx) = session("eof-session");
    let mut handle = pending_handle(session);

    let outcome = forward_loop(&mut incoming_rx, &server, &mut handle).await;
    assert!(matches!(outcome, ForwardResult::Done(None)));
    assert!(handle.shutdown.is_cancelled());
    interrupt_rx.changed().await.unwrap();
    assert_eq!(*interrupt_rx.borrow_and_update(), 2);
    assert!(handle.agent_task.await.unwrap_err().is_cancelled());
}

#[tokio::test]
async fn resignal_allows_agent_to_finish_before_forced_abort() {
    let server = connection();
    let (session, mut interrupt_rx, mut incoming_rx) = session("eof-cooperative-session");
    let agent_task = tokio::spawn(async move {
        while *interrupt_rx.borrow_and_update() < 2 {
            interrupt_rx.changed().await.unwrap();
        }
        None
    });
    let mut handle = handle(session, agent_task);

    let outcome = forward_loop(&mut incoming_rx, &server, &mut handle).await;
    assert!(matches!(outcome, ForwardResult::Done(None)));
    assert!(handle.shutdown.is_cancelled());
}

#[tokio::test]
async fn first_signal_allows_cooperative_agent_to_finish() {
    let server = connection();
    let (session, mut interrupt_rx, mut incoming_rx) = session("eof-first-signal-session");
    let agent_task = tokio::spawn(async move {
        interrupt_rx.changed().await.unwrap();
        None
    });
    let mut handle = handle(session, agent_task);

    let outcome = forward_loop(&mut incoming_rx, &server, &mut handle).await;
    assert!(matches!(outcome, ForwardResult::Done(None)));
    assert!(handle.shutdown.is_cancelled());
}
