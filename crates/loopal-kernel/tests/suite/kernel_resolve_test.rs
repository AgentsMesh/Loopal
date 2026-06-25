use std::collections::HashMap;
use std::sync::Arc;

use loopal_config::{ProviderConfig, ProvidersConfig, Settings};
use loopal_kernel::{Kernel, RoutedSlot, unresolvable_models};
use loopal_provider::{OpenAiProvider, ProviderRegistry};
use loopal_provider_api::{ModelRouter, SharedModelRouter, TaskType};

fn openai_kernel(model: &str, routing: HashMap<TaskType, String>) -> Kernel {
    let settings = Settings {
        model: model.into(),
        model_routing: routing,
        providers: ProvidersConfig {
            anthropic: None,
            openai: Some(ProviderConfig {
                api_key: Some("test-openai-key".into()),
                api_key_env: None,
                base_url: None,
            }),
            google: None,
            openai_compat: vec![],
        },
        ..Default::default()
    };
    Kernel::new(settings).unwrap()
}

#[test]
fn resolve_task_summarization_falls_back_to_main_model() {
    let kernel = openai_kernel("gpt-5.5", HashMap::new());
    let router = ModelRouter::from_parts("gpt-5.5".into(), HashMap::new());
    let (model, _provider) = kernel
        .resolve_task(&router, TaskType::Summarization)
        .expect("summarization falls back to the resolvable main model");
    assert_eq!(model, "gpt-5.5");
}

#[test]
fn resolve_task_errors_when_routed_model_has_no_provider() {
    let mut routing = HashMap::new();
    routing.insert(TaskType::Summarization, "mystery-model-9000".into());
    let kernel = openai_kernel("gpt-5.5", routing.clone());
    let router = ModelRouter::from_parts("gpt-5.5".into(), routing);
    assert!(
        kernel
            .resolve_task(&router, TaskType::Summarization)
            .is_err()
    );
}

#[test]
fn resolve_task_uses_explicit_override_when_provider_present() {
    let mut routing = HashMap::new();
    routing.insert(TaskType::Summarization, "gpt-4o".into());
    let kernel = openai_kernel("gpt-5.5", routing.clone());
    let router = ModelRouter::from_parts("gpt-5.5".into(), routing);
    let (model, _) = kernel
        .resolve_task(&router, TaskType::Summarization)
        .unwrap();
    assert_eq!(model, "gpt-4o");
}

#[test]
fn unresolvable_models_flags_only_the_anthropic_route() {
    let mut routing = HashMap::new();
    routing.insert(TaskType::Summarization, "claude-haiku-4-5-20251001".into());
    routing.insert(TaskType::Refine, "gpt-4o".into());
    let settings = Settings {
        model: "gpt-5.5".into(),
        model_routing: routing,
        ..Default::default()
    };
    let mut registry = ProviderRegistry::new();
    registry.register(Arc::new(OpenAiProvider::new("k".into())));

    let flagged = unresolvable_models(&settings, &registry);
    assert_eq!(flagged.len(), 1);
    assert_eq!(flagged[0].slot, RoutedSlot::Task(TaskType::Summarization));
    assert_eq!(flagged[0].model, "claude-haiku-4-5-20251001");
}

#[test]
fn unresolvable_models_flags_unreachable_main_model() {
    let settings = Settings {
        model: "claude-opus-4-8".into(),
        model_routing: HashMap::new(),
        ..Default::default()
    };
    let mut registry = ProviderRegistry::new();
    registry.register(Arc::new(OpenAiProvider::new("k".into())));

    let flagged = unresolvable_models(&settings, &registry);
    assert_eq!(flagged.len(), 1);
    assert_eq!(flagged[0].slot, RoutedSlot::MainModel);
    assert_eq!(flagged[0].model, "claude-opus-4-8");
}

#[test]
fn unresolvable_models_empty_when_all_resolvable() {
    let settings = Settings {
        model: "gpt-5.5".into(),
        model_routing: HashMap::new(),
        ..Default::default()
    };
    let mut registry = ProviderRegistry::new();
    registry.register(Arc::new(OpenAiProvider::new("k".into())));

    assert!(unresolvable_models(&settings, &registry).is_empty());
}

#[test]
fn resolve_task_via_reader_observes_model_switch() {
    // The classifier resolves through a ModelRouterReader sharing the runner's
    // router; a mid-session /model switch on the writer must be observed here —
    // proving the classifier-stale fix end-to-end through the resolve chokepoint.
    let kernel = openai_kernel("gpt-5.5", HashMap::new());
    let writer = SharedModelRouter::with_default("gpt-5.5".into());
    let reader = writer.reader();

    let (before, _) = kernel
        .resolve_task(&reader.read(), TaskType::Default)
        .unwrap();
    assert_eq!(before, "gpt-5.5");

    writer.set_default("gpt-4o".into());
    let (after, _) = kernel
        .resolve_task(&reader.read(), TaskType::Default)
        .unwrap();
    assert_eq!(
        after, "gpt-4o",
        "reader must see the writer's /model switch"
    );
}

#[test]
fn kernel_unresolvable_models_reports_via_own_settings() {
    // Env-independent: an unknown model resolves to openai_compat, which is
    // never auto-registered (unlike claude-*, which an ANTHROPIC_API_KEY in the
    // environment would silently make resolvable).
    let mut routing = HashMap::new();
    routing.insert(TaskType::Summarization, "mystery-model-9000".into());
    let kernel = openai_kernel("gpt-5.5", routing);
    let flagged = kernel.unresolvable_models();
    assert_eq!(flagged.len(), 1);
    assert_eq!(flagged[0].slot, RoutedSlot::Task(TaskType::Summarization));
    assert_eq!(flagged[0].model, "mystery-model-9000");
}

#[test]
fn unresolvable_models_respects_resolvable_default_override() {
    // Base model is anthropic (unregistered) but model_routing.default overrides
    // it with a registered openai model — the unused base must NOT be flagged.
    let mut routing = HashMap::new();
    routing.insert(TaskType::Default, "gpt-5.5".into());
    let settings = Settings {
        model: "claude-opus-4-8".into(),
        model_routing: routing,
        ..Default::default()
    };
    let mut registry = ProviderRegistry::new();
    registry.register(Arc::new(OpenAiProvider::new("k".into())));

    assert!(unresolvable_models(&settings, &registry).is_empty());
}
