use std::sync::Arc;

#[derive(Clone, Debug)]
pub(crate) struct ConnectionGeneration(Arc<()>);

impl ConnectionGeneration {
    pub(crate) fn new() -> Self {
        Self(Arc::new(()))
    }

    pub(crate) fn is(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
