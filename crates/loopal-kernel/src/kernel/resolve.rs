use std::sync::Arc;

use loopal_error::LoopalError;
use loopal_provider_api::{ModelRouter, Provider, TaskType};

use super::Kernel;

impl Kernel {
    /// Single chokepoint from a `TaskType` to a usable `(model, provider)`
    /// pair: route the task through `router`, then resolve the provider.
    /// Errors loudly when the routed model has no registered provider —
    /// callers must not collapse that into a silent no-op.
    pub fn resolve_task(
        &self,
        router: &ModelRouter,
        task: TaskType,
    ) -> std::result::Result<(String, Arc<dyn Provider>), LoopalError> {
        let model = router.resolve(task).to_string();
        let provider = self.resolve_provider(&model)?;
        Ok((model, provider))
    }

    /// The configured models (`settings.model` + `model_routing`) that resolve
    /// to no registered provider. Pure mechanism — the caller owns the logging
    /// severity and any decision to act on it.
    pub fn unresolvable_models(&self) -> Vec<crate::preflight::UnresolvableModel> {
        crate::preflight::unresolvable_models(&self.settings, &self.provider_registry)
    }
}
