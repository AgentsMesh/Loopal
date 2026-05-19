use std::collections::HashMap;

use loopal_agent_server::testing::build_model_router;
use loopal_config::Settings;
use loopal_provider_api::TaskType;

const HAIKU: &str = "claude-haiku-4-5-20251001";

fn settings_with(model: &str, routing: HashMap<TaskType, String>) -> Settings {
    Settings {
        model: model.into(),
        model_routing: routing,
        ..Settings::default()
    }
}

#[test]
fn summarization_defaults_to_haiku_when_unconfigured() {
    let router = build_model_router(&settings_with("claude-opus-4-7", HashMap::new()));
    assert_eq!(router.resolve(TaskType::Summarization), HAIKU);
}

#[test]
fn user_summarization_model_overrides_default() {
    let mut routing = HashMap::new();
    routing.insert(TaskType::Summarization, "claude-sonnet-4-6".into());
    let router = build_model_router(&settings_with("claude-opus-4-7", routing));
    assert_eq!(router.resolve(TaskType::Summarization), "claude-sonnet-4-6");
}

#[test]
fn default_task_unaffected_by_summarization_injection() {
    let router = build_model_router(&settings_with("claude-opus-4-7", HashMap::new()));
    assert_eq!(router.resolve(TaskType::Default), "claude-opus-4-7");
}

#[test]
fn other_task_types_fall_back_to_default_model() {
    let router = build_model_router(&settings_with("claude-opus-4-7", HashMap::new()));
    assert_eq!(router.resolve(TaskType::Classification), "claude-opus-4-7");
    assert_eq!(router.resolve(TaskType::Refine), "claude-opus-4-7");
}

#[test]
fn empty_routing_still_yields_haiku_for_summarization() {
    let router = build_model_router(&settings_with("anthropic-test-model", HashMap::new()));
    assert_eq!(router.resolve(TaskType::Summarization), HAIKU);
    assert_eq!(router.resolve(TaskType::Default), "anthropic-test-model");
}
