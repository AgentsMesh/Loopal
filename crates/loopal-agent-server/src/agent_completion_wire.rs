use loopal_error::AgentOutput;
use loopal_ipc::connection::{Connection, Listening};
use loopal_ipc::protocol::methods;
use loopal_output_guard::FinalSinkRedactionSeed;
use loopal_protocol::AgentCompletion;

pub(super) fn from_output(
    output: Option<&AgentOutput>,
    seed: &FinalSinkRedactionSeed,
    max_result_bytes: usize,
) -> AgentCompletion {
    let completion = match output {
        Some(output) => AgentCompletion::new(
            output.terminate_reason.as_str(),
            Some(output.result.clone()),
        ),
        None => AgentCompletion::new("shutdown", None),
    };
    seed.guard_completion_with_result_limit(completion, max_result_bytes)
}

pub(super) async fn send(
    connection: &Connection<Listening>,
    output: Option<&AgentOutput>,
    seed: &FinalSinkRedactionSeed,
    max_result_bytes: usize,
) {
    let completion = from_output(output, seed, max_result_bytes);
    let _ = connection
        .send_notification(
            methods::AGENT_COMPLETED.name,
            serde_json::to_value(completion).expect("AgentCompletion is serializable"),
        )
        .await;
}

#[cfg(test)]
mod tests {
    use loopal_error::TerminateReason;
    use loopal_ipc::connection::Incoming;

    use super::*;

    #[test]
    fn wire_guard_rejects_oversized_output() {
        let output = AgentOutput {
            result: "canary".repeat(20_000),
            terminate_reason: TerminateReason::Goal,
        };

        let completion = from_output(
            Some(&output),
            &FinalSinkRedactionSeed::new(),
            loopal_output_guard::MAX_AGENT_COMPLETION_RESULT_BYTES,
        );
        assert_eq!(
            completion.reason,
            loopal_output_guard::OUTPUT_GUARD_REJECTED_REASON
        );
        assert!(!completion.output().contains("canary"));
    }

    #[tokio::test]
    async fn send_emits_only_the_guarded_completion() {
        let (sender_transport, receiver_transport) = loopal_ipc::duplex_pair();
        let (sender, _sender_rx) = Connection::new(sender_transport).into_listening();
        let (_receiver, mut receiver_rx) = Connection::new(receiver_transport).into_listening();
        let output = AgentOutput {
            result: "canary".repeat(20_000),
            terminate_reason: TerminateReason::Goal,
        };

        send(
            &sender,
            Some(&output),
            &FinalSinkRedactionSeed::new(),
            loopal_output_guard::MAX_AGENT_COMPLETION_RESULT_BYTES,
        )
        .await;

        let Incoming::Notification { method, params } = receiver_rx.recv().await.unwrap() else {
            panic!("expected completion notification");
        };
        assert_eq!(method, methods::AGENT_COMPLETED.name);
        let completion: AgentCompletion = serde_json::from_value(params).unwrap();
        assert_eq!(
            completion.reason,
            loopal_output_guard::OUTPUT_GUARD_REJECTED_REASON
        );
        assert!(!completion.output().contains("canary"));
    }

    #[test]
    fn shutdown_completion_has_no_result() {
        let completion = from_output(
            None,
            &FinalSinkRedactionSeed::new(),
            loopal_output_guard::MAX_AGENT_COMPLETION_RESULT_BYTES,
        );

        assert_eq!(completion.reason, "shutdown");
        assert_eq!(completion.result, None);
    }

    #[test]
    fn wire_redacts_session_secret() {
        let seed = FinalSinkRedactionSeed::new();
        seed.observe("access_token", "resolved-token".into())
            .unwrap();
        let output = AgentOutput {
            result: "token=resolved-token".into(),
            terminate_reason: TerminateReason::Goal,
        };

        let completion = from_output(
            Some(&output),
            &seed,
            loopal_output_guard::MAX_AGENT_COMPLETION_RESULT_BYTES,
        );
        assert_eq!(completion.output(), "token=<secret_ref:access_token>");
    }

    #[test]
    fn workflow_wire_honors_its_trusted_result_limit() {
        let limit = loopal_output_guard::MAX_AGENT_COMPLETION_RESULT_BYTES + 1;
        let output = AgentOutput {
            result: "w".repeat(limit),
            terminate_reason: TerminateReason::Goal,
        };

        let completion = from_output(Some(&output), &FinalSinkRedactionSeed::new(), limit);
        assert_eq!(completion.reason, "goal");
        assert_eq!(completion.result.as_deref(), Some(output.result.as_str()));
    }
}
