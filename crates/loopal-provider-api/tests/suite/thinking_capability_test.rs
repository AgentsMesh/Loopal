//! Tests for ThinkingCapability methods.

use loopal_provider_api::ThinkingCapability;

#[test]
fn anthropic_capabilities_forbid_prefill() {
    assert!(ThinkingCapability::BudgetRequired.forbids_prefill());
    assert!(ThinkingCapability::Adaptive.forbids_prefill());
}

#[test]
fn non_anthropic_capabilities_allow_prefill() {
    assert!(!ThinkingCapability::None.forbids_prefill());
    assert!(!ThinkingCapability::ReasoningEffort.forbids_prefill());
    assert!(!ThinkingCapability::ThinkingBudget.forbids_prefill());
}
