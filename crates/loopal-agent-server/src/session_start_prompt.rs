use loopal_protocol::{Envelope, MessageSource, QualifiedAddress, UserContent};
use loopal_runtime::agent_input::AgentInput;

use crate::session_hub::SharedSession;

pub(super) async fn push_prompt_envelope(session: &SharedSession, prompt: &str, has_fork: bool) {
    let text = if has_fork {
        format!("{}\n\n{prompt}", loopal_context::fork::FORK_BOILERPLATE)
    } else {
        prompt.to_string()
    };
    let envelope = Envelope::new(
        MessageSource::Human,
        QualifiedAddress::local(loopal_protocol::ROOT_AGENT_NAME),
        UserContent::text_only(text),
    );
    if let Err(e) = session.input_tx.send(AgentInput::Message(envelope)).await {
        tracing::warn!(error = %e, "failed to enqueue headless prompt envelope");
    }
}
