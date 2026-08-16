use loopal_protocol::QualifiedAddress;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WorkflowOwner {
    pub session_id: String,
    pub root_agent: QualifiedAddress,
}

impl WorkflowOwner {
    pub fn new(session_id: impl Into<String>, root_agent: QualifiedAddress) -> Self {
        Self {
            session_id: session_id.into(),
            root_agent,
        }
    }

    pub(crate) fn is_valid(&self) -> bool {
        valid_component(&self.session_id)
            && self.root_agent.is_local()
            && valid_component(&self.root_agent.agent)
    }
}

fn valid_component(value: &str) -> bool {
    !value.is_empty()
        && !matches!(value, "." | "..")
        && !value.contains(['/', '\\'])
        && value.len() <= 128
}
