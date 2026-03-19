use std::collections::HashMap;
use std::path::{Path, PathBuf};

use loopal_types::sandbox::{ResolvedPolicy, SandboxPolicy};

use crate::env_sanitizer::sanitize_current_env;
use crate::platform;

/// Sandboxed command ready for execution (crate-internal type).
#[derive(Debug, Clone)]
pub struct SandboxedCommand {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: HashMap<String, String>,
}

/// Wrap a shell command string into a `SandboxedCommand` with OS-level
/// sandboxing and sanitized environment variables.
///
/// When the sandbox policy is `Disabled`, returns a plain `sh -c` command
/// with sanitized env only.
pub fn wrap_command(
    policy: &ResolvedPolicy,
    command: &str,
    cwd: &Path,
) -> SandboxedCommand {
    let env = sanitize_current_env();

    if policy.policy == SandboxPolicy::Disabled {
        return SandboxedCommand {
            program: "sh".into(),
            args: vec!["-c".into(), command.into()],
            cwd: cwd.to_path_buf(),
            env,
        };
    }

    // Try OS-level sandbox wrapping
    match platform::build_sandbox_prefix(policy, cwd) {
        Some((program, mut prefix_args)) => {
            prefix_args.extend_from_slice(&[
                "sh".into(),
                "-c".into(),
                command.into(),
            ]);
            SandboxedCommand {
                program,
                args: prefix_args,
                cwd: cwd.to_path_buf(),
                env,
            }
        }
        None => {
            SandboxedCommand {
                program: "sh".into(),
                args: vec!["-c".into(), command.into()],
                cwd: cwd.to_path_buf(),
                env,
            }
        }
    }
}
