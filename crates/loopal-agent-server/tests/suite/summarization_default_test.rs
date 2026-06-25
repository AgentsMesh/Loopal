use std::collections::HashMap;

use loopal_agent_server::testing::build_model_router;
use loopal_config::Settings;
use loopal_provider_api::TaskType;

fn settings_with(model: &str, routing: HashMap<TaskType, String>) -> Settings {
    Settings {
        model: model.into(),
        model_routing: routing,
        ..Settings::default()
    }
}

#[test]
fn summarization_falls_back_to_main_model_when_unconfigured() {
    let router = build_model_router(&settings_with("claude-opus-4-7", HashMap::new()));
    assert_eq!(router.resolve(TaskType::Summarization), "claude-opus-4-7");
}

#[test]
fn user_summarization_model_overrides_default() {
    let mut routing = HashMap::new();
    routing.insert(TaskType::Summarization, "claude-sonnet-4-6".into());
    let router = build_model_router(&settings_with("claude-opus-4-7", routing));
    assert_eq!(router.resolve(TaskType::Summarization), "claude-sonnet-4-6");
}

#[test]
fn default_task_resolves_to_main_model() {
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
fn gpt_only_deployment_summarizes_with_its_main_model() {
    let router = build_model_router(&settings_with("gpt-5.5", HashMap::new()));
    assert_eq!(router.resolve(TaskType::Summarization), "gpt-5.5");
    assert_eq!(router.resolve(TaskType::Default), "gpt-5.5");
}
