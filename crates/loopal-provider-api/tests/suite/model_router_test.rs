use loopal_provider_api::{ModelRouter, TaskType};
use std::collections::HashMap;

#[test]
fn test_new_defaults_to_given_model() {
    let router = ModelRouter::new("claude-sonnet-4-6".into());
    assert_eq!(router.default_model(), "claude-sonnet-4-6");
}

#[test]
fn test_resolve_default_returns_main_model() {
    let router = ModelRouter::new("claude-sonnet-4-6".into());
    assert_eq!(router.resolve(TaskType::Default), "claude-sonnet-4-6");
}

#[test]
fn test_resolve_summarization_falls_back_to_default() {
    let router = ModelRouter::new("claude-sonnet-4-6".into());
    assert_eq!(router.resolve(TaskType::Summarization), "claude-sonnet-4-6");
}

#[test]
fn test_from_parts_with_summarization_override() {
    let mut routing = HashMap::new();
    routing.insert(TaskType::Summarization, "claude-haiku-3-5-20241022".into());
    let router = ModelRouter::from_parts("claude-sonnet-4-6".into(), routing);

    assert_eq!(router.resolve(TaskType::Default), "claude-sonnet-4-6");
    assert_eq!(
        router.resolve(TaskType::Summarization),
        "claude-haiku-3-5-20241022"
    );
}

#[test]
fn test_set_default_updates_main_model() {
    let mut router = ModelRouter::new("claude-sonnet-4-6".into());
    router.set_default("claude-opus-4-6".into());

    assert_eq!(router.default_model(), "claude-opus-4-6");
    assert_eq!(router.resolve(TaskType::Default), "claude-opus-4-6");
}

#[test]
fn test_set_default_does_not_affect_overrides() {
    let mut routing = HashMap::new();
    routing.insert(TaskType::Summarization, "claude-haiku-3-5-20241022".into());
    let mut router = ModelRouter::from_parts("claude-sonnet-4-6".into(), routing);

    router.set_default("claude-opus-4-6".into());

    // Override stays, only default changes
    assert_eq!(
        router.resolve(TaskType::Summarization),
        "claude-haiku-3-5-20241022"
    );
    assert_eq!(router.resolve(TaskType::Default), "claude-opus-4-6");
}

#[test]
fn test_from_parts_empty_routing() {
    let router = ModelRouter::from_parts("gpt-4o".into(), HashMap::new());
    assert_eq!(router.resolve(TaskType::Default), "gpt-4o");
    assert_eq!(router.resolve(TaskType::Summarization), "gpt-4o");
}

#[test]
fn test_set_default_clears_default_override() {
    let mut routing = HashMap::new();
    routing.insert(TaskType::Default, "claude-opus-4-6".into());
    routing.insert(TaskType::Summarization, "claude-haiku-3-5-20241022".into());
    let mut router = ModelRouter::from_parts("claude-sonnet-4-6".into(), routing);

    assert_eq!(router.resolve(TaskType::Default), "claude-opus-4-6");

    // Runtime /model switch clears Default override
    router.set_default("gpt-4o".into());

    assert_eq!(router.resolve(TaskType::Default), "gpt-4o");
    assert_eq!(
        router.resolve(TaskType::Summarization),
        "claude-haiku-3-5-20241022"
    );
}
