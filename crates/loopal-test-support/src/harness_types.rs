use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;

use loopal_protocol::{AgentEvent, ControlCommand, Envelope};
use loopal_runtime::agent_loop::AgentLoopRunner;
use loopal_session::SessionController;
use loopal_turn::Turn;

use crate::fixture::TestFixture;

pub struct IntegrationHarness {
    pub runner: AgentLoopRunner,
    pub event_rx: mpsc::Receiver<AgentEvent>,
    pub mailbox_tx: mpsc::Sender<Envelope>,
    pub control_tx: mpsc::Sender<ControlCommand>,
    pub interrupt: loopal_protocol::InterruptSignal,
    pub session_ctrl: SessionController,
    pub fixture: TestFixture,
    pub recorded_messages: Arc<Mutex<Vec<Vec<Turn>>>>,
}

impl IntegrationHarness {
    pub(crate) fn from_parts(h: SpawnedHarness, runner: AgentLoopRunner) -> Self {
        Self {
            runner,
            event_rx: h.event_rx,
            mailbox_tx: h.mailbox_tx,
            control_tx: h.control_tx,
            interrupt: h.interrupt,
            session_ctrl: h.session_ctrl,
            fixture: h.fixture,
            recorded_messages: h.recorded_messages,
        }
    }
}

pub struct SpawnedHarness {
    pub event_tx: mpsc::Sender<AgentEvent>,
    pub event_rx: mpsc::Receiver<AgentEvent>,
    pub mailbox_tx: mpsc::Sender<Envelope>,
    pub control_tx: mpsc::Sender<ControlCommand>,
    pub interrupt: loopal_protocol::InterruptSignal,
    pub session_ctrl: SessionController,
    pub fixture: TestFixture,
    pub recorded_messages: Arc<Mutex<Vec<Vec<Turn>>>>,
}
