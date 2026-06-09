use std::sync::Arc;

use loopal_config::SandboxPolicy;

use super::Kernel;

impl Kernel {
    pub fn create_backend(
        &self,
        cwd: &std::path::Path,
        session_id: &str,
    ) -> Arc<dyn loopal_tool_api::Backend> {
        let sandbox = self.sandbox.read().expect("sandbox lock poisoned").clone();
        let policy = if sandbox.policy != SandboxPolicy::Disabled {
            Some(loopal_sandbox::resolve_policy(&sandbox, cwd))
        } else {
            None
        };
        let limits = loopal_backend::ResourceLimits {
            image_max_bytes: self.settings.images.max_bytes,
            image_max_pixels: self.settings.images.max_pixels,
            ..loopal_backend::ResourceLimits::default()
        };
        loopal_backend::LocalBackend::new(cwd.to_path_buf(), policy, limits, session_id)
    }

    pub fn set_sandbox_policy(&self, policy: SandboxPolicy) {
        self.sandbox.write().expect("sandbox lock poisoned").policy = policy;
    }

    pub fn sandbox_policy(&self) -> SandboxPolicy {
        self.sandbox.read().expect("sandbox lock poisoned").policy
    }
}
