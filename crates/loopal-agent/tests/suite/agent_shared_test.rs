use std::sync::Arc;

use loopal_agent::shared::AgentShared;
use loopal_config::{SandboxPolicy, Settings};
use loopal_test_support::TestFixture;
use loopal_test_support::agent_ctx::agent_tool_context_with_settings;

fn shared_with_policy(fixture: &TestFixture, policy: SandboxPolicy) -> Arc<AgentShared> {
    let mut settings = Settings::default();
    settings.sandbox.policy = policy;
    let (_ctx, shared) = agent_tool_context_with_settings(fixture, settings);
    shared
}

#[test]
fn no_sandbox_true_when_policy_disabled() {
    let fixture = TestFixture::new();
    let shared = shared_with_policy(&fixture, SandboxPolicy::Disabled);
    assert!(shared.no_sandbox());
}

#[test]
fn no_sandbox_false_when_policy_default_write() {
    let fixture = TestFixture::new();
    let shared = shared_with_policy(&fixture, SandboxPolicy::DefaultWrite);
    assert!(!shared.no_sandbox());
}

#[test]
fn no_sandbox_false_when_policy_read_only() {
    let fixture = TestFixture::new();
    let shared = shared_with_policy(&fixture, SandboxPolicy::ReadOnly);
    assert!(!shared.no_sandbox());
}
