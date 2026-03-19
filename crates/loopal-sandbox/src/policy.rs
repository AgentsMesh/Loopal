use std::path::{Path, PathBuf};

use loopal_types::sandbox::{
    ResolvedPolicy, SandboxConfig, SandboxPolicy,
};

use crate::sensitive_patterns::SENSITIVE_FILE_GLOBS;

/// Resolve a `SandboxConfig` from settings into a `ResolvedPolicy`
/// by combining user config with defaults and workspace context.
pub fn resolve_policy(
    config: &SandboxConfig,
    cwd: &Path,
) -> ResolvedPolicy {
    if config.policy == SandboxPolicy::Disabled {
        return ResolvedPolicy {
            policy: SandboxPolicy::Disabled,
            writable_paths: Vec::new(),
            deny_write_globs: Vec::new(),
            deny_read_globs: Vec::new(),
            network: config.network.clone(),
        };
    }

    // Writable paths: cwd + tmpdir + user-configured extras
    let mut writable_paths = Vec::new();
    // Canonicalize all writable paths to handle symlinks (e.g. macOS /tmp → /private/tmp)
    let add_canonical = |paths: &mut Vec<PathBuf>, p: PathBuf| {
        if let Ok(canon) = p.canonicalize() {
            paths.push(canon);
        } else {
            paths.push(p);
        }
    };
    add_canonical(&mut writable_paths, cwd.to_path_buf());
    if let Ok(tmp) = std::env::var("TMPDIR") {
        add_canonical(&mut writable_paths, tmp.into());
    }
    add_canonical(&mut writable_paths, std::env::temp_dir());

    // Add user-configured writable paths (resolved relative to cwd)
    for pattern in &config.filesystem.allow_write {
        let path = Path::new(pattern);
        if path.is_absolute() {
            add_canonical(&mut writable_paths, path.to_path_buf());
        } else {
            add_canonical(&mut writable_paths, cwd.join(path));
        }
    }

    // Deny-write globs: defaults + user config
    let mut deny_write_globs: Vec<String> = SENSITIVE_FILE_GLOBS
        .iter()
        .map(|s| s.to_string())
        .collect();
    deny_write_globs.extend(config.filesystem.deny_write.clone());

    // Deny-read globs: user config only (no defaults — reads are open)
    let deny_read_globs = config.filesystem.deny_read.clone();

    ResolvedPolicy {
        policy: config.policy,
        writable_paths,
        deny_write_globs,
        deny_read_globs,
        network: config.network.clone(),
    }
}
