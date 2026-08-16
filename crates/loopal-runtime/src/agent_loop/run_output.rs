use loopal_error::{AgentOutput, TerminateReason};

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    pub(super) fn guarded_output(
        &self,
        last_output: String,
        last_error: Option<String>,
        terminate_reason: TerminateReason,
    ) -> AgentOutput {
        let result = match last_error {
            Some(error) if last_output.is_empty() => error,
            _ => last_output,
        };
        let seed = self
            .params
            .deps
            .kernel
            .secret_client()
            .and_then(|client| client.final_sink_redaction_seed());
        crate::agent_output_guard::guard(
            AgentOutput {
                result,
                terminate_reason,
            },
            seed.as_ref(),
        )
    }
}
