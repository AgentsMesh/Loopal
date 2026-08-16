use loopal_config::{SandboxConfig, SandboxPolicy};

use super::build_command;

#[test]
fn resolved_policy_builds_a_sanitized_shell_command() {
    let config = SandboxConfig {
        policy: SandboxPolicy::Disabled,
        ..SandboxConfig::default()
    };
    let policy = loopal_sandbox::resolve_policy(&config, std::path::Path::new("/tmp"));

    let (program, args, env) =
        build_command(std::path::Path::new("/tmp"), Some(&policy), "printf safe");
    assert_eq!(program, "sh");
    assert_eq!(args, ["-c", "printf safe"]);
    assert!(env.is_some());
}
