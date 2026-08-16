use loopal_protocol::{
    ControlCommand, Envelope, WorkflowTerminalDisposition, WorkflowTerminalNotification,
};
use tokio::sync::{mpsc, watch};

/// Input to the agent loop — either a data message or a control command.
///
/// Replaces the former `UserCommand` enum by preserving the full `Envelope`
/// (with source/target/id/timestamp) instead of flattening to a plain string.
/// Control commands pass through without adaptation.
///
/// `Envelope` carries qualified addresses (variable hub paths), so the
/// variant sizes diverge — but boxing on every input would add a heap
/// allocation in the hot dispatch path. Allow the size difference instead.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum AgentInput {
    /// A data-plane message (human, agent, or channel).
    Message(Envelope),
    /// A control-plane command (mode switch, clear, compact, model switch).
    Control(ControlCommand),
    /// A control command carrying an application acknowledgement lease.
    TrackedControl(ControlRequest),
    /// A terminal workflow result whose acknowledgement follows durable turn persistence.
    WorkflowTerminal(WorkflowTerminalRequest),
}

#[derive(Debug, Clone)]
pub struct WorkflowTerminalRequest {
    notification: WorkflowTerminalNotification,
    acknowledgement: watch::Sender<Option<WorkflowTerminalDisposition>>,
}

impl WorkflowTerminalRequest {
    pub fn tracked(
        notification: WorkflowTerminalNotification,
    ) -> (Self, watch::Receiver<Option<WorkflowTerminalDisposition>>) {
        let (acknowledgement, receiver) = watch::channel(None);
        (
            Self {
                notification,
                acknowledgement,
            },
            receiver,
        )
    }

    pub fn notification(&self) -> &WorkflowTerminalNotification {
        &self.notification
    }

    pub async fn acknowledge(&self, disposition: WorkflowTerminalDisposition) {
        self.acknowledgement.send_replace(Some(disposition));
    }
}

#[derive(Debug, Clone)]
pub struct ControlRequest {
    command: ControlCommand,
    /// Receiver lifetime is an execution lease. The RPC caller may already
    /// have received `{status:"queued"}` while a late-ack keeper still owns
    /// the receiver so the accepted command remains eligible for application.
    acknowledgement: mpsc::Sender<ControlAcknowledgement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlAcknowledgement {
    Applied,
    Rejected(String),
}

impl ControlRequest {
    pub fn tracked(command: ControlCommand) -> (Self, mpsc::Receiver<ControlAcknowledgement>) {
        let (acknowledgement, receiver) = mpsc::channel(1);
        (
            Self {
                command,
                acknowledgement,
            },
            receiver,
        )
    }

    pub fn command(&self) -> &ControlCommand {
        &self.command
    }

    pub fn application_is_live(&self) -> bool {
        !self.acknowledgement.is_closed()
    }

    pub async fn acknowledge(&self, outcome: ControlAcknowledgement) {
        let _ = self.acknowledgement.send(outcome).await;
    }
}
