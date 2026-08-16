use loopal_output_guard::FinalSinkRedactionSeed;
use loopal_protocol::AgentEvent;

pub(super) fn guard(event: AgentEvent, seed: &FinalSinkRedactionSeed) -> AgentEvent {
    seed.guard_event(event)
}

#[cfg(test)]
mod tests {
    use loopal_output_guard::MAX_AGENT_EVENT_PAYLOAD_BYTES;
    use loopal_protocol::AgentEventPayload;

    use super::*;

    #[test]
    fn oversized_event_becomes_content_free_error() {
        let event = AgentEvent::root(AgentEventPayload::Stream {
            text: "canary".repeat(MAX_AGENT_EVENT_PAYLOAD_BYTES / 6 + 1),
        });

        let guarded = guard(event, &FinalSinkRedactionSeed::new());
        assert!(matches!(guarded.payload, AgentEventPayload::Error { .. }));
        let encoded = serde_json::to_string(&guarded).unwrap();
        assert!(!encoded.contains("canary"));
    }

    #[test]
    fn observed_secret_is_redacted() {
        let seed = FinalSinkRedactionSeed::new();
        seed.observe("token", "session-secret".into()).unwrap();
        let event = AgentEvent::root(AgentEventPayload::Stream {
            text: "value=session-secret".into(),
        });

        let guarded = guard(event, &seed);
        let encoded = serde_json::to_string(&guarded).unwrap();
        assert!(encoded.contains("<secret_ref:token>"));
        assert!(!encoded.contains("session-secret"));
    }
}
