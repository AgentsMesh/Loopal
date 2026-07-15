use loopal_protocol::{Envelope, MessageSource, QualifiedAddress, UserContent};
use loopal_runtime::agent_input::AgentInput;

use crate::agent_setup_helpers::build_fork_synthetic_turn;
use crate::params::StartParams;
use crate::session_hub::SharedSession;

pub(super) async fn push_start_prompt(session: &SharedSession, start: &StartParams) {
    let Some(prompt) = &start.prompt else {
        return;
    };
    if build_fork_synthetic_turn(start).is_some() {
        return;
    }
    let envelope = Envelope::new(
        MessageSource::Human,
        QualifiedAddress::local(loopal_protocol::ROOT_AGENT_NAME),
        UserContent::text_only(prompt.clone()),
    );
    if let Err(e) = session.input_tx.send(AgentInput::Message(envelope)).await {
        tracing::warn!(error = %e, "failed to enqueue headless prompt envelope");
    }
}
