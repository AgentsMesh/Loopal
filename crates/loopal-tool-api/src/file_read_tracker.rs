use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;

use crate::path::ResolvedPath;

/// Per-session record of the content the model has observed for each file. Used
/// to refuse a blind overwrite of a file that changed on disk since it was read
/// (e.g. edited by the user or a concurrent agent process). Only a content hash
/// is kept, so memory stays bounded regardless of file size or count.
#[derive(Default)]
pub struct FileReadTracker {
    seen: Mutex<HashMap<ResolvedPath, u64>>,
}

impl FileReadTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record the content the model just observed at `path`.
    pub fn record(&self, path: &ResolvedPath, content: &str) {
        self.seen
            .lock()
            .unwrap()
            .insert(path.clone(), hash(content));
    }

    /// True if `path` was recorded earlier and its content now differs — i.e. it
    /// changed out from under the agent since it was last read.
    pub fn is_stale(&self, path: &ResolvedPath, current: &str) -> bool {
        let seen = self.seen.lock().unwrap();
        matches!(seen.get(path), Some(&recorded) if recorded != hash(current))
    }
}

fn hash(content: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}
