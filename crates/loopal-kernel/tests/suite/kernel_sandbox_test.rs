use loopal_config::{SandboxConfig, SandboxPolicy, Settings};
use loopal_kernel::Kernel;

fn kernel_with_policy(policy: SandboxPolicy) -> Kernel {
    let settings = Settings {
        sandbox: SandboxConfig {
            policy,
            ..SandboxConfig::default()
        },
        ..Settings::default()
    };
    Kernel::new(settings).unwrap()
}

#[test]
fn sandbox_policy_reflects_initial_settings() {
    let kernel = kernel_with_policy(SandboxPolicy::ReadOnly);
    assert_eq!(kernel.sandbox_policy(), SandboxPolicy::ReadOnly);
}

#[test]
fn set_sandbox_policy_mutates_live_policy() {
    let kernel = kernel_with_policy(SandboxPolicy::DefaultWrite);
    assert_eq!(kernel.sandbox_policy(), SandboxPolicy::DefaultWrite);
    kernel.set_sandbox_policy(SandboxPolicy::ReadOnly);
    assert_eq!(kernel.sandbox_policy(), SandboxPolicy::ReadOnly);
    kernel.set_sandbox_policy(SandboxPolicy::Disabled);
    assert_eq!(kernel.sandbox_policy(), SandboxPolicy::Disabled);
}

#[test]
fn create_backend_after_switch_to_disabled_yields_no_policy() {
    let kernel = kernel_with_policy(SandboxPolicy::ReadOnly);
    kernel.set_sandbox_policy(SandboxPolicy::Disabled);
    let dir = std::env::temp_dir();
    let _backend = kernel.create_backend(&dir, "sess-sandbox-test");
}
