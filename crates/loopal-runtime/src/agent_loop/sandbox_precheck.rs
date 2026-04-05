//! Sandbox path pre-check: extract paths from tool input, check sandbox
//! policy, and determine if approval is needed before tool execution.

use loopal_tool_api::Backend;
use serde_json::Value;

/// A path that requires sandbox approval before the tool can execute.
pub struct ApprovalNeeded {
    pub path: String,
    /// Whether the operation is a write (vs read). Reserved for future use
    /// when command-level and network-level approval share this struct.
    #[allow(dead_code)]
    pub is_write: bool,
    pub reason: String,
}

/// Extract file paths from a tool's input based on tool name conventions.
///
/// Returns `Vec<(raw_path, is_write)>`.  Tools with opaque path semantics
/// (Bash, Glob, Grep, MCP, etc.) return an empty list — the execution-time
/// fallback in `LocalBackend::resolve_checked` handles those.
pub fn extract_paths(tool_name: &str, input: &Value) -> Vec<(String, bool)> {
    match tool_name {
        "Write" | "Edit" | "MultiEdit" => single(input, "file_path", true),
        "Read" => single(input, "file_path", false),
        "Delete" => single(input, "path", true),
        "MoveFile" => {
            let mut v = single(input, "src", true);
            v.extend(single(input, "dst", true));
            v
        }
        "CopyFile" => {
            let mut v = single(input, "src", false);
            v.extend(single(input, "dst", true));
            v
        }
        "ApplyPatch" => patch_paths(input),
        _ => Vec::new(),
    }
}

/// Check extracted paths against the sandbox, returning any that need approval.
pub fn check_paths(backend: &dyn Backend, paths: &[(String, bool)]) -> Vec<ApprovalNeeded> {
    paths
        .iter()
        .filter_map(|(raw, is_write)| {
            backend
                .check_sandbox_path(raw, *is_write)
                .map(|reason| ApprovalNeeded {
                    path: raw.clone(),
                    is_write: *is_write,
                    reason,
                })
        })
        .collect()
}

/// Approve all paths from `needs` via `backend.approve_path()`.
pub fn approve_all(backend: &dyn Backend, needs: &[ApprovalNeeded]) {
    for n in needs {
        let p = std::path::Path::new(&n.path);
        let abs = if p.is_absolute() {
            p.to_path_buf()
        } else {
            backend.cwd().join(p)
        };
        backend.approve_path(&abs);
    }
}

fn single(input: &Value, key: &str, is_write: bool) -> Vec<(String, bool)> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| vec![(s.to_string(), is_write)])
        .unwrap_or_default()
}

fn patch_paths(input: &Value) -> Vec<(String, bool)> {
    let patch = match input.get("patch").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return Vec::new(),
    };
    patch
        .lines()
        .filter(|l| l.starts_with("*** "))
        .filter_map(|l| {
            l.strip_prefix("*** ").map(|p| {
                // Strip trailing timestamp (unified diff: `*** file\ttimestamp`).
                let path = p.split('\t').next().unwrap_or(p).trim();
                (path.to_string(), true)
            })
        })
        .filter(|(p, _)| !p.is_empty())
        .collect()
}
