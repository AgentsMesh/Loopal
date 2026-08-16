use loopal_output_guard::FinalSinkRedactionSeed;
use loopal_protocol::{AgentCompletion, Envelope, UserContent};

pub(crate) fn guard(
    completion: AgentCompletion,
    redaction_seed: &FinalSinkRedactionSeed,
) -> AgentCompletion {
    redaction_seed.guard_completion(completion)
}

pub(crate) fn guard_with_result_limit(
    completion: AgentCompletion,
    redaction_seed: &FinalSinkRedactionSeed,
    max_result_bytes: usize,
) -> AgentCompletion {
    redaction_seed.guard_completion_with_result_limit(completion, max_result_bytes)
}

pub(crate) fn canonicalize_agent_result(
    mut envelope: Envelope,
    completion: AgentCompletion,
    redaction_seed: &FinalSinkRedactionSeed,
) -> (Envelope, AgentCompletion) {
    let completion = guard(completion, redaction_seed);
    envelope.content = UserContent::text_only(completion.output());
    envelope.summary = None;
    envelope.agent_completion = Some(completion.clone());
    (envelope, completion)
}

#[cfg(test)]
mod tests {
    use loopal_protocol::{MessageSource, QualifiedAddress};

    use super::*;

    #[test]
    fn canonicalization_rejects_large_result_and_drops_untrusted_extras() {
        let completion = AgentCompletion::goal(Some("canary".repeat(20_000)));
        let mut envelope = Envelope::new(
            MessageSource::AgentResult {
                child: QualifiedAddress::local("worker"),
            },
            QualifiedAddress::local("parent"),
            "different raw content",
        );
        envelope.summary = Some("raw summary".into());
        envelope
            .content
            .images
            .push(loopal_protocol::user_content::ImageAttachment {
                media_type: "image/png".into(),
                data: "raw-image".into(),
            });

        let (envelope, completion) =
            canonicalize_agent_result(envelope, completion, &FinalSinkRedactionSeed::new());
        assert_eq!(
            completion.reason,
            loopal_output_guard::OUTPUT_GUARD_REJECTED_REASON
        );
        assert_eq!(envelope.content.text, completion.output());
        assert!(envelope.content.images.is_empty());
        assert!(envelope.summary.is_none());
        assert_eq!(envelope.agent_completion, Some(completion));
        assert!(!envelope.content.text.contains("canary"));
    }
}
