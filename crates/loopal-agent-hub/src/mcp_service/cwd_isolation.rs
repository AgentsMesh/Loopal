use std::path::{Path, PathBuf};

use loopal_config::McpServerConfig;

pub(super) fn inject(
    server_name: &str,
    cfg: McpServerConfig,
    cwd: &Path,
) -> McpServerConfig {
    let Some(iso) = cfg.cwd_isolation().cloned() else {
        return cfg;
    };
    let McpServerConfig::Stdio {
        command,
        mut args,
        env,
        enabled,
        timeout_ms,
        sharing,
        cwd_isolation,
    } = cfg
    else {
        return cfg;
    };
    let subdir = iso.cache_subdir.as_deref().unwrap_or(server_name);
    let isolated_dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(subdir)
        .join(cwd_hash(cwd));
    inject_arg_if_absent(&mut args, &iso.arg, &isolated_dir.to_string_lossy());
    McpServerConfig::Stdio {
        command,
        args,
        env,
        enabled,
        timeout_ms,
        sharing,
        cwd_isolation,
    }
}

fn inject_arg_if_absent(args: &mut Vec<String>, flag: &str, value: &str) {
    if args.iter().any(|a| a.starts_with(flag)) {
        return;
    }
    args.push(format!("{flag}={value}"));
}

pub(super) fn cwd_hash(cwd: &Path) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    cwd.hash(&mut h);
    format!("{:016x}", h.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use loopal_config::{CwdIsolation, McpSharing};

    fn iso(arg: &str, subdir: Option<&str>) -> CwdIsolation {
        CwdIsolation {
            arg: arg.to_string(),
            cache_subdir: subdir.map(String::from),
        }
    }

    #[test]
    fn cwd_hash_is_deterministic() {
        let p = PathBuf::from("/a/b/c");
        assert_eq!(cwd_hash(&p), cwd_hash(&p));
    }

    #[test]
    fn cwd_hash_differs_for_different_paths() {
        let a = PathBuf::from("/proj-a");
        let b = PathBuf::from("/proj-b");
        assert_ne!(cwd_hash(&a), cwd_hash(&b));
    }

    #[test]
    fn inject_arg_skips_existing_flag() {
        let mut args = vec!["--user-data-dir=/custom".to_string()];
        inject_arg_if_absent(&mut args, "--user-data-dir", "/should-not-add");
        assert_eq!(args.len(), 1);
        assert_eq!(args[0], "--user-data-dir=/custom");
    }

    #[test]
    fn inject_arg_appends_when_missing() {
        let mut args: Vec<String> = vec![];
        inject_arg_if_absent(&mut args, "--user-data-dir", "/new");
        assert_eq!(args, vec!["--user-data-dir=/new"]);
    }

    #[test]
    fn server_with_isolation_gets_cwd_scoped_dir() {
        let cwd = PathBuf::from("/proj-x");
        let cfg = McpServerConfig::Stdio {
            command: "npx".into(),
            args: vec!["-y".into(), "chrome-devtools-mcp@latest".into()],
            env: Default::default(),
            enabled: true,
            timeout_ms: 30_000,
            sharing: McpSharing::HubSingleton,
            cwd_isolation: Some(iso("--user-data-dir", Some("chrome-devtools-mcp"))),
        };
        let isolated = inject("chrome", cfg, &cwd);
        if let McpServerConfig::Stdio { args, .. } = isolated {
            assert!(args.iter().any(|a| a.starts_with("--user-data-dir=")));
            assert!(args.iter().any(|a| a.contains(&cwd_hash(&cwd))));
            assert!(args.iter().any(|a| a.contains("chrome-devtools-mcp")));
        } else {
            panic!("expected Stdio config");
        }
    }

    #[test]
    fn server_without_isolation_passes_through_unchanged() {
        let cwd = PathBuf::from("/proj-y");
        let cfg = McpServerConfig::Stdio {
            command: "other-mcp".into(),
            args: vec!["--port".into(), "8080".into()],
            env: Default::default(),
            enabled: true,
            timeout_ms: 30_000,
            sharing: McpSharing::HubSingleton,
            cwd_isolation: None,
        };
        let passed_through = inject("other-mcp", cfg, &cwd);
        if let McpServerConfig::Stdio { args, .. } = passed_through {
            assert_eq!(args, vec!["--port".to_string(), "8080".to_string()]);
        }
    }

    #[test]
    fn cache_subdir_falls_back_to_server_name() {
        let cwd = PathBuf::from("/proj-z");
        let cfg = McpServerConfig::Stdio {
            command: "thing".into(),
            args: vec![],
            env: Default::default(),
            enabled: true,
            timeout_ms: 30_000,
            sharing: McpSharing::HubSingleton,
            cwd_isolation: Some(iso("--data-dir", None)),
        };
        let isolated = inject("my-server", cfg, &cwd);
        if let McpServerConfig::Stdio { args, .. } = isolated {
            assert!(args.iter().any(|a| a.contains("my-server")));
        } else {
            panic!("expected Stdio");
        }
    }
}
