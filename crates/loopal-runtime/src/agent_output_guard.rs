use loopal_error::{AgentOutput, TerminateReason};
use loopal_output_guard::{
    FinalSinkRedactionSeed, FinalSinkRedactionSeedError, guard_or_reject_agent_completion,
};
use loopal_protocol::AgentCompletion;
use secrecy::SecretString;

pub(crate) fn guard(output: AgentOutput, seed: Option<&FinalSinkRedactionSeed>) -> AgentOutput {
    guard_with_snapshot(
        output,
        seed.map(FinalSinkRedactionSeed::snapshot).transpose(),
    )
}

fn guard_with_snapshot(
    output: AgentOutput,
    snapshot: Result<Option<Vec<(String, SecretString)>>, FinalSinkRedactionSeedError>,
) -> AgentOutput {
    let snapshot = match snapshot {
        Ok(Some(snapshot)) => snapshot,
        Ok(None) => Vec::new(),
        Err(_) => return rejected_output(),
    };
    let completion = guard_or_reject_agent_completion(
        AgentCompletion::new(output.terminate_reason.as_str(), Some(output.result)),
        &snapshot,
    )
    .into_completion();
    if completion.reason == loopal_output_guard::OUTPUT_GUARD_REJECTED_REASON {
        AgentOutput {
            result: completion.output().to_string(),
            terminate_reason: TerminateReason::Error,
        }
    } else {
        AgentOutput {
            result: completion.result.unwrap_or_default(),
            terminate_reason: output.terminate_reason,
        }
    }
}

fn rejected_output() -> AgentOutput {
    let result = loopal_output_guard::OUTPUT_GUARD_REJECTED_RESULT.into();
    AgentOutput {
        result,
        terminate_reason: TerminateReason::Error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_output_becomes_content_free_failure() {
        let output = guard(
            AgentOutput {
                result: "canary".repeat(20_000),
                terminate_reason: TerminateReason::Goal,
            },
            None,
        );

        assert_eq!(output.terminate_reason, TerminateReason::Error);
        assert!(!output.result.contains("canary"));
    }

    #[test]
    fn session_seed_redacts_resolved_secret_from_completion() {
        let seed = FinalSinkRedactionSeed::new();
        seed.observe("api_key", "resolved-secret".into()).unwrap();

        let output = guard(
            AgentOutput {
                result: "result: resolved-secret".into(),
                terminate_reason: TerminateReason::Goal,
            },
            Some(&seed),
        );

        assert_eq!(output.result, "result: <secret_ref:api_key>");
    }

    #[test]
    fn unavailable_seed_fails_closed() {
        let output = guard_with_snapshot(
            AgentOutput {
                result: "must not escape".into(),
                terminate_reason: TerminateReason::Goal,
            },
            Err(FinalSinkRedactionSeedError),
        );

        assert_eq!(output.terminate_reason, TerminateReason::Error);
        assert_eq!(
            output.result,
            loopal_output_guard::OUTPUT_GUARD_REJECTED_RESULT
        );
    }
}
