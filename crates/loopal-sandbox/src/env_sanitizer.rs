use std::collections::HashMap;

use crate::sensitive_patterns::{SAFE_ENV_ALLOWLIST, SENSITIVE_ENV_PATTERNS};

/// Sanitize environment variables by removing sensitive entries.
///
/// Returns a clean HashMap suitable for passing to a sandboxed subprocess.
/// Variables on the safe allowlist are always kept. Variables matching any
/// sensitive pattern are removed. Unrecognized variables are kept by default.
pub fn sanitize_env(env: &HashMap<String, String>) -> HashMap<String, String> {
    env.iter()
        .filter(|(key, _)| !is_sensitive(key))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// Collect the current process environment, sanitized.
pub fn sanitize_current_env() -> HashMap<String, String> {
    let current: HashMap<String, String> = std::env::vars().collect();
    sanitize_env(&current)
}

/// Check if an environment variable name matches a sensitive pattern.
pub fn is_sensitive(name: &str) -> bool {
    // Safe allowlist always passes
    if SAFE_ENV_ALLOWLIST.contains(&name) {
        return false;
    }
    let upper = name.to_uppercase();
    SENSITIVE_ENV_PATTERNS
        .iter()
        .any(|pattern| upper.contains(pattern))
}

/// Return the list of sensitive variable names found in the given environment.
pub fn find_sensitive_vars(env: &HashMap<String, String>) -> Vec<String> {
    env.keys()
        .filter(|k| is_sensitive(k))
        .cloned()
        .collect()
}
