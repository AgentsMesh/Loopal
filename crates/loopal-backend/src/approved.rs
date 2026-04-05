//! Session-scoped set of sandbox-approved paths.
//!
//! Once a path is approved (user confirmation or Bypass mode), subsequent
//! operations on it skip the `RequiresApproval` sandbox check within the
//! same session.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use parking_lot::RwLock;

/// Thread-safe approved-paths set with interior mutability.
///
/// Wrapped in `RwLock` so it can live inside `Arc<LocalBackend>` without
/// requiring `&mut self`.  The contention profile (rare writes after first
/// approval, frequent reads) is ideal for reader-writer locks.
pub struct ApprovedPaths {
    inner: RwLock<HashSet<PathBuf>>,
}

impl Default for ApprovedPaths {
    fn default() -> Self {
        Self::new()
    }
}

impl ApprovedPaths {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashSet::new()),
        }
    }

    pub fn insert(&self, path: PathBuf) {
        self.inner.write().insert(path);
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.inner.read().contains(path)
    }
}
