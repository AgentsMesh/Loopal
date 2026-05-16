use std::collections::HashMap;

use loopal_tool_api::backend_types::EnvOverride;

const ENV_KEY_BLACKLIST: &[&str] = &[
    "PATH",
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "DYLD_INSERT_LIBRARIES",
    "DYLD_LIBRARY_PATH",
    "HOME",
];

pub(crate) fn is_valid_env_key(s: &str) -> bool {
    let mut chars = s.chars();
    let first = match chars.next() {
        Some(c) => c,
        None => return false,
    };
    if !first.is_ascii_uppercase() && first != '_' {
        return false;
    }
    chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

pub(crate) fn validate_env(env: Option<&HashMap<String, String>>) -> Option<String> {
    let env = env?;
    for k in env.keys() {
        if !is_valid_env_key(k) {
            return Some(format!("env key '{k}' must match ^[A-Z_][A-Z0-9_]*$"));
        }
        if ENV_KEY_BLACKLIST.iter().any(|b| *b == k) {
            return Some(format!(
                "env key '{k}' is blacklisted (PATH/LD_*/DYLD_*/HOME) to preserve sandbox"
            ));
        }
    }
    None
}

pub(crate) fn build_env_override(env: Option<&HashMap<String, String>>) -> EnvOverride {
    let Some(env) = env else {
        return EnvOverride::default();
    };
    let mut out = EnvOverride::new();
    for (k, v) in env {
        out = out.with(k.clone(), v);
    }
    out
}
