use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

pub struct SpawnRegistry {
    entries: RwLock<HashMap<String, SpawnEntry>>,
}

#[derive(Debug, Clone)]
struct SpawnEntry {
    cwd: PathBuf,
    parent_name: Option<String>,
}

const MAX_PARENT_HOPS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WalkError {
    NotFound,
    CycleDetected,
}

impl SpawnRegistry {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }

    pub fn register(&self, agent_name: String, cwd: PathBuf, parent_name: Option<String>) {
        let canonical = cwd.canonicalize().unwrap_or(cwd);
        let mut w = self.entries.write().unwrap();
        w.insert(
            agent_name,
            SpawnEntry {
                cwd: canonical,
                parent_name,
            },
        );
    }

    pub fn unregister(&self, agent_name: &str) -> bool {
        self.entries.write().unwrap().remove(agent_name).is_some()
    }

    pub fn cwd_of(&self, agent_name: &str) -> Option<PathBuf> {
        self.entries
            .read()
            .unwrap()
            .get(agent_name)
            .map(|e| e.cwd.clone())
    }

    pub fn parent_of(&self, agent_name: &str) -> Option<String> {
        self.entries
            .read()
            .unwrap()
            .get(agent_name)
            .and_then(|e| e.parent_name.clone())
    }

    pub fn is_root(&self, agent_name: &str) -> bool {
        self.entries
            .read()
            .unwrap()
            .get(agent_name)
            .map(|e| e.parent_name.is_none())
            .unwrap_or(false)
    }

    pub fn root_of(&self, agent_name: &str) -> Option<String> {
        let entries = self.entries.read().unwrap();
        walk_to_root(&entries, agent_name, |name, _| name.to_string()).ok()
    }

    /// Verify a caller agent may access a vault rooted at `target_cwd`.
    /// Walks `caller_name` up the parent chain to its spawn root; access is
    /// granted only when target_cwd and root_cwd are on the same path
    /// branch (one is ancestor or descendant of the other, equal counts).
    pub fn verify_vault_access(&self, caller_name: &str, target_cwd: &Path) -> bool {
        let target = match target_cwd.canonicalize() {
            Ok(p) => p,
            Err(_) => return false,
        };
        let entries = self.entries.read().unwrap();
        let root_cwd = match walk_to_root(&entries, caller_name, |_, e| e.cwd.clone()) {
            Ok(p) => p,
            Err(WalkError::NotFound) => return false,
            Err(WalkError::CycleDetected) => {
                tracing::error!(
                    caller = %caller_name,
                    "vault access denied: parent chain cycle detected — \
                     this is a config/wiring bug, not a permission denial"
                );
                return false;
            }
        };
        root_cwd.starts_with(&target) || target.starts_with(&root_cwd)
    }
}

impl Default for SpawnRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn walk_to_root<R>(
    entries: &HashMap<String, SpawnEntry>,
    start: &str,
    extract: impl FnOnce(&str, &SpawnEntry) -> R,
) -> Result<R, WalkError> {
    let mut current = start;
    for _ in 0..MAX_PARENT_HOPS {
        let entry = entries.get(current).ok_or(WalkError::NotFound)?;
        match &entry.parent_name {
            None => return Ok(extract(current, entry)),
            Some(p) => current = p.as_str(),
        }
    }
    tracing::warn!(
        agent_name = %start,
        max_hops = MAX_PARENT_HOPS,
        "spawn registry: parent chain exceeded max hops (cycle detected)"
    );
    Err(WalkError::CycleDetected)
}
