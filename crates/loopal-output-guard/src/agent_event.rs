use std::io::{self, Write};

use loopal_protocol::{AgentEvent, AgentEventPayload};
use secrecy::SecretString;
use thiserror::Error;

use crate::{JsonGuardError, OutputGuard, OutputGuardBuildError};

pub const MAX_AGENT_EVENT_PAYLOAD_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct GuardedAgentEvent(AgentEvent);

impl GuardedAgentEvent {
    pub fn into_event(self) -> AgentEvent {
        self.0
    }
}

#[derive(Debug, Error)]
pub enum AgentEventGuardError {
    #[error("agent event redactor unavailable")]
    Build(#[from] OutputGuardBuildError),
    #[error("agent event serialization failed")]
    Serialize(#[source] serde_json::Error),
    #[error("agent event payload rejected")]
    Payload(#[source] JsonGuardError),
    #[error("agent event deserialization failed")]
    Deserialize(#[source] serde_json::Error),
}

pub fn guard_agent_event(
    mut event: AgentEvent,
    seed: &[(String, SecretString)],
) -> Result<GuardedAgentEvent, AgentEventGuardError> {
    if seed.is_empty() {
        let encoded_bytes = encoded_len(&event.payload)?;
        if encoded_bytes > MAX_AGENT_EVENT_PAYLOAD_BYTES {
            return Err(AgentEventGuardError::Payload(
                JsonGuardError::EncodedByteLimitExceeded {
                    actual_bytes: encoded_bytes,
                    max_bytes: MAX_AGENT_EVENT_PAYLOAD_BYTES,
                },
            ));
        }
        return Ok(GuardedAgentEvent(event));
    }
    let value = serde_json::to_value(&event.payload).map_err(AgentEventGuardError::Serialize)?;
    let payload = OutputGuard::new(seed)?
        .guard_json(&value, MAX_AGENT_EVENT_PAYLOAD_BYTES)
        .map_err(AgentEventGuardError::Payload)?
        .into_inner()
        .into_value();
    event.payload = serde_json::from_value::<AgentEventPayload>(payload)
        .map_err(AgentEventGuardError::Deserialize)?;
    Ok(GuardedAgentEvent(event))
}

fn encoded_len(payload: &AgentEventPayload) -> Result<usize, AgentEventGuardError> {
    let mut writer = CountingWriter::default();
    serde_json::to_writer(&mut writer, payload).map_err(AgentEventGuardError::Serialize)?;
    writer.flush().expect("counting writer flush is infallible");
    Ok(writer.bytes)
}

#[derive(Default)]
struct CountingWriter {
    bytes: usize,
}

impl Write for CountingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes = self.bytes.saturating_add(bytes.len());
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub fn guard_or_reject_agent_event(
    event: AgentEvent,
    seed: &[(String, SecretString)],
) -> GuardedAgentEvent {
    let fallback = rejected_agent_event(&event);
    guard_agent_event(event, seed).unwrap_or(GuardedAgentEvent(fallback))
}

pub fn rejected_agent_event(event: &AgentEvent) -> AgentEvent {
    AgentEvent {
        agent_name: event.agent_name.clone(),
        event_id: event.event_id,
        turn_id: event.turn_id,
        correlation_id: event.correlation_id,
        rev: event.rev,
        routing_generation: event.routing_generation,
        payload: AgentEventPayload::Error {
            message: "agent event rejected by output guard".into(),
        },
    }
}
