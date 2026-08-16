use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const OWNED_BOUNDARIES: &[&str] = &[
    "crates/loopal-acp/src/",
    "crates/loopal-agent/src/",
    "crates/loopal-agent-client/src/",
    "crates/loopal-agent-hub/src/",
    "crates/loopal-agent-server/src/",
    "crates/loopal-backend/src/",
    "crates/loopal-config/src/",
    "crates/loopal-hub-vault/src/",
    "crates/loopal-ipc/src/",
    "crates/loopal-mcp/src/",
    "crates/loopal-output-guard/src/",
    "crates/loopal-protocol/src/",
    "crates/loopal-provider-api/src/",
    "crates/loopal-runtime/src/",
    "crates/loopal-secret-client/src/",
    "crates/loopal-secret-runtime/src/",
    "crates/loopal-session/src/",
    "crates/loopal-storage/src/",
    "crates/loopal-tool-api/src/",
    "crates/loopal-turn/src/",
    "crates/loopal-tui/src/",
    "crates/tools/process/",
    "crates/loopal-vault-age/src/",
    "crates/loopal-vault-api/src/",
    "crates/loopal-view-state/src/",
    "crates/loopal-workflow-schema/src/",
    "crates/tools/filesystem/",
    "src/bootstrap/",
];

pub fn review(
    changed: impl IntoIterator<Item = String>,
    included: &BTreeSet<String>,
    exclusions: &Path,
    workspace: &Path,
) -> Result<(), Vec<String>> {
    let exclusions = load_exclusions(exclusions).map_err(|error| vec![error])?;
    let mut errors = Vec::new();
    for path in changed.into_iter().filter(|path| review_candidate(path)) {
        let full_path = workspace.join(&path);
        if !full_path.exists() {
            if included.contains(&path) {
                errors.push(format!("included source was deleted: {path}"));
            }
            continue;
        }
        if included.contains(&path) {
            continue;
        }
        let Some((expected_hash, rationale)) = exclusions.get(&path) else {
            errors.push(format!(
                "changed Stage 0/workflow source is unreviewed: {path}"
            ));
            continue;
        };
        match hash_file(&full_path) {
            Ok(actual) if &actual == expected_hash => {
                if rationale.trim().len() < 12 {
                    errors.push(format!("scope exclusion rationale is too short: {path}"));
                }
            }
            Ok(actual) => errors.push(format!(
                "scope exclusion is stale for {path}: expected {expected_hash}, got {actual}"
            )),
            Err(error) => errors.push(error),
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn review_candidate(path: &str) -> bool {
    path.ends_with(".rs")
        && OWNED_BOUNDARIES
            .iter()
            .any(|prefix| path.starts_with(prefix))
        && !path.split('/').any(is_test_component)
}

pub fn hash_file(path: &Path) -> Result<String, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    Ok(format!("{hash:016x}"))
}

fn load_exclusions(
    path: &Path,
) -> Result<std::collections::BTreeMap<String, (String, String)>, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let mut values = std::collections::BTreeMap::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.splitn(3, '|');
        let path = parts.next().unwrap_or_default();
        let hash = parts.next().unwrap_or_default();
        let rationale = parts.next().unwrap_or_default();
        if !review_candidate(path) || hash.len() != 16 || rationale.is_empty() {
            return Err(format!("line {}: malformed scope exclusion", index + 1));
        }
        if values
            .insert(path.into(), (hash.into(), rationale.into()))
            .is_some()
        {
            return Err(format!(
                "line {}: duplicate scope exclusion: {path}",
                index + 1
            ));
        }
    }
    Ok(values)
}

fn is_test_component(value: &str) -> bool {
    let stem = value.strip_suffix(".rs").unwrap_or(value);
    matches!(stem, "test" | "tests" | "testing" | "fixtures" | "e2e")
        || stem.starts_with("test_")
        || stem.starts_with("tests_")
        || stem.ends_with("_test")
        || stem.ends_with("_tests")
}
