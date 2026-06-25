use loopal_config::Settings;
use loopal_provider::ProviderRegistry;
use loopal_provider_api::TaskType;

/// Which routing slot a model came from. Distinguishes the base `settings.model`
/// from a per-task `model_routing` entry without overloading `Option`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutedSlot {
    MainModel,
    Task(TaskType),
}

/// A configured model that resolves to no registered provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvableModel {
    pub slot: RoutedSlot,
    pub model: String,
}

/// Collect every configured model (`settings.model` plus each `model_routing`
/// value) that resolves to no registered provider. Pure over its inputs so it
/// is unit-testable without a full Kernel; callers decide how loudly to report.
pub fn unresolvable_models(
    settings: &Settings,
    registry: &ProviderRegistry,
) -> Vec<UnresolvableModel> {
    let mut out = Vec::new();
    // Effective main model: a `model_routing[Default]` override wins over
    // `settings.model` (mirrors `ModelRouter::resolve`), so validate that — else
    // a resolvable override would false-flag the (unused) overridden base.
    let main = settings
        .model_routing
        .get(&TaskType::Default)
        .unwrap_or(&settings.model);
    if !registry.is_model_resolvable(main) {
        out.push(UnresolvableModel {
            slot: RoutedSlot::MainModel,
            model: main.clone(),
        });
    }
    for (task, model) in &settings.model_routing {
        if *task == TaskType::Default {
            continue; // folded into the main-model check above
        }
        if !registry.is_model_resolvable(model) {
            out.push(UnresolvableModel {
                slot: RoutedSlot::Task(*task),
                model: model.clone(),
            });
        }
    }
    out
}
