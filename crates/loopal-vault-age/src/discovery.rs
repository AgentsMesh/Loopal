use std::path::Path;

/// Enumerate vault names under a vaults directory, by scanning for
/// `<name>.vault/store.age` files. Returns sorted names. Empty when the
/// directory doesn't exist, isn't readable, or contains no initialized vaults.
///
/// This is the single source of truth for "which vaults exist on disk" —
/// CLI list, runtime auto-discovery, and config resolution all call it.
pub fn list_initialized_vaults(vaults_dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(vaults_dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|e| {
            let n = e.file_name().into_string().ok()?;
            let name = n.strip_suffix(".vault")?.to_string();
            let store = vaults_dir.join(&n).join("store.age");
            store.exists().then_some(name)
        })
        .collect();
    names.sort();
    names
}
