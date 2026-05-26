use std::path::{Path, PathBuf};

use regex::Regex;
use walkdir::WalkDir;

const SELF_PATH: &str = "tests/architecture_boundary_test.rs";
const SCAN_ROOTS: &[&str] = &["crates", "src", "tests", "benchmarks"];

fn is_excluded_dir(name: &str) -> bool {
    matches!(name, "target" | ".git" | "node_modules") || name.starts_with("bazel-")
}

fn workspace_root() -> PathBuf {
    if let Ok(dir) = std::env::var("BUILD_WORKSPACE_DIRECTORY") {
        return PathBuf::from(dir);
    }
    let mut p = std::env::current_exe().expect("exe path");
    while p.pop() {
        if SCAN_ROOTS.iter().all(|r| p.join(r).exists())
            || p.join("MODULE.bazel").exists()
            || p.join("WORKSPACE").exists()
        {
            return p;
        }
    }
    PathBuf::from(".")
}

fn iter_rs_files(root: &Path) -> impl Iterator<Item = PathBuf> + '_ {
    WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| !is_excluded_dir(&e.file_name().to_string_lossy()))
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "rs"))
}

/// Scan source tree for `pattern`; fail if any match falls outside `allowed`.
/// Test file itself is always excluded (it carries the forbidden patterns in
/// regex literals).
fn grep_forbidden(pattern: &str, allowed_path_prefixes: &[&str], hint: &str) {
    let re = Regex::new(pattern).expect("invalid regex");
    let root = workspace_root();
    let mut scanned = 0usize;
    let mut violations: Vec<String> = Vec::new();

    for scan_root in SCAN_ROOTS {
        let scan_path = root.join(scan_root);
        if !scan_path.exists() {
            continue;
        }
        for path in iter_rs_files(&scan_path) {
            scanned += 1;
            let rel = path.strip_prefix(&root).unwrap_or(&path);
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            if rel_str == SELF_PATH {
                continue;
            }
            if allowed_path_prefixes.iter().any(|p| rel_str.starts_with(p)) {
                continue;
            }
            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            for (line_no, line) in content.lines().enumerate() {
                if re.is_match(line) {
                    violations.push(format!("{}:{}: {}", rel_str, line_no + 1, line.trim()));
                }
            }
        }
    }

    assert!(
        scanned > 0,
        "architecture_boundary_test scanned 0 files from {} — workspace root not detected",
        root.display()
    );
    assert!(
        violations.is_empty(),
        "Architectural boundary violation: {hint}\n\nViolations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn loopal_message_crate_stays_dead() {
    grep_forbidden(
        r"loopal[-_]message",
        &[],
        "loopal-message was retired (PR-6). Wire-format types live in loopal-provider-api + loopal-turn.",
    );
}

#[test]
fn context_store_from_messages_stays_removed() {
    grep_forbidden(
        r"ContextStore::from_messages\b",
        &[],
        "ContextStore::from_messages was deleted. Use loopal_test_support::seed_history or TurnTracker.",
    );
}

#[test]
fn turn_store_mutation_stays_internal() {
    grep_forbidden(
        r"\.store_mut\(\)|TurnStore::turns_mut\b",
        &[],
        "TurnStore mutation must route through TurnTracker. Direct store_mut / turns_mut are removed.",
    );
}

/// Deny-list: leaf crates that must NEVER touch wire-format types
/// (MessageRole/ContentBlock). Anything outside this list is allowed.
#[test]
fn wire_format_types_stay_out_of_leaf_crates() {
    let forbidden_prefixes = &[
        "crates/loopal-error/",
        "crates/loopal-protocol/",
        "crates/loopal-ipc/",
        "crates/loopal-storage/",
        "crates/loopal-session/",
        "crates/loopal-config/",
        "crates/loopal-vault-api/",
        "crates/loopal-vault-age/",
        "crates/loopal-hub-vault/",
        "crates/loopal-secret-runtime/",
        "crates/loopal-decision-api/",
        "crates/loopal-tool-api/",
        "crates/loopal-hooks/",
        "crates/loopal-git/",
        "crates/loopal-backend/",
        "crates/loopal-telemetry/",
    ];
    let re = Regex::new(r"\b(MessageRole|ContentBlock)\b").unwrap();
    let root = workspace_root();
    let mut violations = Vec::new();
    for prefix in forbidden_prefixes {
        let scan_path = root.join(prefix);
        if !scan_path.exists() {
            continue;
        }
        for path in iter_rs_files(&scan_path) {
            let rel = path.strip_prefix(&root).unwrap_or(&path);
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            for (line_no, line) in content.lines().enumerate() {
                if re.is_match(line) {
                    violations.push(format!("{}:{}: {}", rel_str, line_no + 1, line.trim()));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "Wire-format types (MessageRole/ContentBlock) leaked into leaf crates:\n{}",
        violations.join("\n")
    );
}
