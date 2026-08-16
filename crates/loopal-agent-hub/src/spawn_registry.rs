use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use crate::types::AgentExecutionRef;

#[path = "spawn_registry_lease.rs"]
mod lease;

pub struct SpawnRegistry {
    entries: RwLock<HashMap<String, SpawnEntry>>,
}

#[derive(Debug, Clone)]
struct SpawnEntry {
    execution: AgentExecutionRef,
    cwd: PathBuf,
    parent: Option<AgentExecutionRef>,
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
        let execution = AgentExecutionRef::local(agent_name, 0);
        let parent = parent_name.map(|name| AgentExecutionRef::local(name, 0));
        self.register_exact(execution, cwd, parent);
    }

    pub(crate) fn register_exact(
        &self,
        execution: AgentExecutionRef,
        cwd: PathBuf,
        parent: Option<AgentExecutionRef>,
    ) -> bool {
        let canonical = cwd.canonicalize().unwrap_or(cwd);
        let mut entries = self.entries.write().unwrap();
        if entries
            .get(&execution.address.agent)
            .is_some_and(|current| {
                current.execution.connection_generation > execution.connection_generation
            })
        {
            return false;
        }
        entries.insert(
            execution.address.agent.clone(),
            SpawnEntry {
                execution,
                cwd: canonical,
                parent,
            },
        );
        true
    }

    pub fn unregister(&self, agent_name: &str) -> bool {
        self.entries.write().unwrap().remove(agent_name).is_some()
    }

    pub(crate) fn unregister_exact(&self, execution: &AgentExecutionRef) -> bool {
        let mut entries = self.entries.write().unwrap();
        if entries
            .get(&execution.address.agent)
            .is_some_and(|entry| entry.execution == *execution)
        {
            entries.remove(&execution.address.agent);
            true
        } else {
            false
        }
    }

    pub fn cwd_of(&self, agent_name: &str) -> Option<PathBuf> {
        self.entries
            .read()
            .unwrap()
            .get(agent_name)
            .map(|entry| entry.cwd.clone())
    }

    pub(crate) fn cwd_for(&self, execution: &AgentExecutionRef) -> Option<PathBuf> {
        self.entries
            .read()
            .unwrap()
            .get(&execution.address.agent)
            .filter(|entry| entry.execution == *execution)
            .map(|entry| entry.cwd.clone())
    }

    pub fn parent_of(&self, agent_name: &str) -> Option<String> {
        self.entries
            .read()
            .unwrap()
            .get(agent_name)
            .and_then(|entry| entry.parent.as_ref())
            .map(|parent| parent.address.agent.clone())
    }

    pub fn is_root(&self, agent_name: &str) -> bool {
        self.entries
            .read()
            .unwrap()
            .get(agent_name)
            .is_some_and(|entry| entry.parent.is_none())
    }

    pub fn root_of(&self, agent_name: &str) -> Option<String> {
        let entries = self.entries.read().unwrap();
        let start = entries.get(agent_name)?.execution.clone();
        walk_to_root(&entries, &start, |entry| {
            entry.execution.address.agent.clone()
        })
        .ok()
    }

    pub(crate) fn root_execution(
        &self,
        execution: &AgentExecutionRef,
    ) -> Option<AgentExecutionRef> {
        let entries = self.entries.read().unwrap();
        walk_to_root(&entries, execution, |entry| entry.execution.clone()).ok()
    }

    pub fn verify_vault_access(&self, caller_name: &str, target_cwd: &Path) -> bool {
        let entries = self.entries.read().unwrap();
        let Some(start) = entries
            .get(caller_name)
            .map(|entry| entry.execution.clone())
        else {
            return false;
        };
        verify_access(&entries, &start, target_cwd)
    }

    pub(crate) fn verify_vault_access_exact(
        &self,
        execution: &AgentExecutionRef,
        target_cwd: &Path,
    ) -> bool {
        verify_access(&self.entries.read().unwrap(), execution, target_cwd)
    }
}

impl Default for SpawnRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn verify_access(
    entries: &HashMap<String, SpawnEntry>,
    start: &AgentExecutionRef,
    target_cwd: &Path,
) -> bool {
    let Ok(target) = target_cwd.canonicalize() else {
        return false;
    };
    let root_cwd = match walk_to_root(entries, start, |entry| entry.cwd.clone()) {
        Ok(path) => path,
        Err(WalkError::NotFound) => return false,
        Err(WalkError::CycleDetected) => {
            tracing::error!(caller = %start.address, "vault access denied: spawn parent cycle");
            return false;
        }
    };
    root_cwd.starts_with(&target) || target.starts_with(&root_cwd)
}

fn walk_to_root<R>(
    entries: &HashMap<String, SpawnEntry>,
    start: &AgentExecutionRef,
    extract: impl FnOnce(&SpawnEntry) -> R,
) -> Result<R, WalkError> {
    let mut current = start.clone();
    for _ in 0..MAX_PARENT_HOPS {
        let entry = entries
            .get(&current.address.agent)
            .filter(|entry| entry.execution == current)
            .ok_or(WalkError::NotFound)?;
        match &entry.parent {
            None => return Ok(extract(entry)),
            Some(parent) => current = parent.clone(),
        }
    }
    tracing::warn!(agent = %start.address, max_hops = MAX_PARENT_HOPS, "spawn parent cycle detected");
    Err(WalkError::CycleDetected)
}
