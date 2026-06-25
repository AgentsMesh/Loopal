use std::collections::HashMap;
use std::sync::{Arc, RwLock, RwLockReadGuard};

use crate::TaskType;

/// Routes task types to specific model IDs.
///
/// Resolution: task-specific override → default model. No hardcoded
/// per-task fallbacks — the user picks the summarization model in
/// `settings.json` (model_routing.summarization).
#[derive(Debug, Clone)]
pub struct ModelRouter {
    default_model: String,
    overrides: HashMap<TaskType, String>,
}

impl ModelRouter {
    pub fn new(default_model: String) -> Self {
        Self {
            default_model,
            overrides: HashMap::new(),
        }
    }

    pub fn from_parts(default_model: String, routing: HashMap<TaskType, String>) -> Self {
        Self {
            default_model,
            overrides: routing,
        }
    }

    pub fn resolve(&self, task: TaskType) -> &str {
        self.overrides
            .get(&task)
            .map(String::as_str)
            .unwrap_or(&self.default_model)
    }

    pub fn default_model(&self) -> &str {
        &self.default_model
    }

    /// Update the default model (e.g., on runtime `/model` switch).
    /// Also clears any `TaskType::Default` override so the new default takes effect.
    pub fn set_default(&mut self, model: String) {
        self.overrides.remove(&crate::TaskType::Default);
        self.default_model = model;
    }
}

/// Shared, interior-mutable handle to a single `ModelRouter` instance.
///
/// One `SharedModelRouter` is the single source of truth per agent: the agent
/// loop's `/model` switch writes through it, and every resolver (compaction,
/// classifier) reads through the same `Arc`, so a mid-session model change is
/// observed everywhere instead of leaving stale clones behind.
#[derive(Clone)]
pub struct SharedModelRouter(Arc<RwLock<ModelRouter>>);

impl SharedModelRouter {
    pub fn new(router: ModelRouter) -> Self {
        Self(Arc::new(RwLock::new(router)))
    }

    pub fn from_parts(default_model: String, routing: HashMap<TaskType, String>) -> Self {
        Self::new(ModelRouter::from_parts(default_model, routing))
    }

    pub fn with_default(default_model: String) -> Self {
        Self::new(ModelRouter::new(default_model))
    }

    /// Borrow the inner router for a synchronous read (e.g. to pass to
    /// `Kernel::resolve_task`). The guard must not be held across `.await`.
    pub fn read(&self) -> RwLockReadGuard<'_, ModelRouter> {
        self.0.read().expect("model router lock poisoned")
    }

    pub fn set_default(&self, model: String) {
        self.0
            .write()
            .expect("model router lock poisoned")
            .set_default(model);
    }

    pub fn model(&self) -> String {
        self.read().resolve(TaskType::Default).to_string()
    }

    /// A read-only handle onto the *same* router instance. Hand this to
    /// resolvers that must observe `/model` switches but must never write —
    /// the write capability stays with the agent loop's `SharedModelRouter`.
    pub fn reader(&self) -> ModelRouterReader {
        ModelRouterReader(self.0.clone())
    }
}

/// Read-only view of a shared `ModelRouter`. Shares the underlying `Arc` with
/// the owning `SharedModelRouter`, so it always sees the latest `/model`
/// switch, but exposes no mutator — single-writer is enforced by type.
#[derive(Clone)]
pub struct ModelRouterReader(Arc<RwLock<ModelRouter>>);

impl ModelRouterReader {
    pub fn read(&self) -> RwLockReadGuard<'_, ModelRouter> {
        self.0.read().expect("model router lock poisoned")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_router_clones_observe_set_default() {
        let a = SharedModelRouter::with_default("model-a".into());
        let b = a.clone();
        assert_eq!(a.model(), "model-a");
        assert_eq!(b.model(), "model-a");
        // Mutating through one clone is visible through the other: single
        // source of truth, no stale copies (the classifier-stale fix).
        b.set_default("model-b".into());
        assert_eq!(a.model(), "model-b");
        assert_eq!(b.read().resolve(TaskType::Default), "model-b");
    }

    #[test]
    fn read_only_handle_observes_writer_set_default() {
        let writer = SharedModelRouter::with_default("m1".into());
        let reader = writer.reader();
        assert_eq!(reader.read().resolve(TaskType::Default), "m1");
        // The runner writes via `set_default`; the classifier's read-only
        // handle sees it (same Arc) without ever holding write capability.
        writer.set_default("m2".into());
        assert_eq!(reader.read().resolve(TaskType::Default), "m2");
    }

    #[test]
    fn from_parts_carries_routing_and_with_default_falls_back() {
        let mut routing = std::collections::HashMap::new();
        routing.insert(TaskType::Summarization, "sum-model".into());
        let r = SharedModelRouter::from_parts("main".into(), routing);
        assert_eq!(r.model(), "main");
        assert_eq!(r.read().resolve(TaskType::Summarization), "sum-model");

        let d = SharedModelRouter::with_default("only".into());
        assert_eq!(d.model(), "only");
        // No overrides → every task falls back to the default model.
        assert_eq!(d.read().resolve(TaskType::Summarization), "only");
    }
}
