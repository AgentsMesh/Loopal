//! Agent registry — manages agent connections, lifecycle, routing.
//!
//! Contains only agent-related state. UI client management is in `UiDispatcher`.

mod completion;
mod events;
mod insertion;
mod operations;
mod queries;
mod registration;
mod removal;
mod shadows;
mod tombstones;
mod topology_queries;

#[cfg(test)]
mod lease_tests;
#[cfg(test)]
mod query_tests;
#[cfg(test)]
mod registration_tests;

pub use completion::PendingCompletionDelivery;

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use tokio::sync::{mpsc, watch};

use loopal_ipc::connection::{Connection, Listening};
use loopal_output_guard::FinalSinkRedactionSeed;
use loopal_protocol::{AgentCompletion, AgentEvent};

use crate::topology::AgentInfo;
use crate::types::{AgentConnectionState, CompletedAgent, LocalChannels, ManagedAgent};

pub const MAX_COMPLETED_AGENTS: usize = 128;

/// Pure agent registry — no UI client knowledge.
pub struct AgentRegistry {
    pub(crate) agents: HashMap<String, ManagedAgent>,
    pub(crate) event_tx: mpsc::Sender<AgentEvent>,
    pub(crate) completions: HashMap<String, watch::Sender<Option<AgentCompletion>>>,
    pub(crate) completed: HashMap<String, CompletedAgent>,
    pub(crate) completed_order: VecDeque<String>,
    pub(crate) completed_limit: usize,
    pub(crate) next_generation: u64,
    /// TCP agent names reserved during the register-ACK handshake. Reservations
    /// are neither routable nor visible in topology snapshots.
    pub(crate) reservations: HashMap<String, Arc<Connection<Listening>>>,
    pub(crate) final_sink_redaction_seed: FinalSinkRedactionSeed,
}

impl AgentRegistry {
    pub fn new(event_tx: mpsc::Sender<AgentEvent>) -> Self {
        Self::new_with_redaction_seed(event_tx, FinalSinkRedactionSeed::new())
    }

    pub fn new_with_redaction_seed(
        event_tx: mpsc::Sender<AgentEvent>,
        final_sink_redaction_seed: FinalSinkRedactionSeed,
    ) -> Self {
        Self {
            agents: HashMap::new(),
            event_tx,
            completions: HashMap::new(),
            completed: HashMap::new(),
            completed_order: VecDeque::new(),
            completed_limit: MAX_COMPLETED_AGENTS,
            next_generation: 1,
            reservations: HashMap::new(),
            final_sink_redaction_seed,
        }
    }

    pub fn event_sender(&self) -> mpsc::Sender<AgentEvent> {
        self.event_tx.clone()
    }

    pub fn set_local(&mut self, name: &str, channels: LocalChannels) {
        self.forget_completed(name);
        let generation = self.allocate_generation();
        let view = ManagedAgent::new_view_reducer(name);
        self.agents.insert(
            name.to_string(),
            ManagedAgent {
                state: AgentConnectionState::Local(channels),
                info: AgentInfo::new(name, None, None),
                runtime: None,
                parent_generation: None,
                completion_tx: None,
                notify_parent_on_completion: true,
                view,
                completion: None,
                admitted_error: false,
                generation,
            },
        );
    }

    pub(crate) fn allocate_generation(&mut self) -> u64 {
        let generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        generation
    }
}
