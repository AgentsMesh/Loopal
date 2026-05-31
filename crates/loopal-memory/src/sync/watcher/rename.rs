use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::extract::slug_from_path;
use crate::store::MemoryGraph;
use crate::sync::relative_path;

pub async fn detect(
    graph: &MemoryGraph,
    base: &Path,
    deletes: &[PathBuf],
    updates: &[PathBuf],
) -> Vec<(PathBuf, PathBuf)> {
    if deletes.is_empty() || updates.is_empty() {
        return Vec::new();
    }

    let mut new_hashes: HashMap<String, PathBuf> = HashMap::new();
    for path in updates {
        if let Ok(content) = tokio::fs::read_to_string(path).await {
            new_hashes.insert(sha256_hex(&content), path.clone());
        }
    }

    let mut renames = Vec::new();
    let mut consumed: HashSet<PathBuf> = HashSet::new();
    for old_path in deletes {
        let rel = relative_path(base, old_path);
        let old_slug = slug_from_path(&rel);
        let Ok(Some(old_node)) = graph.get_node(&old_slug).await else {
            continue;
        };
        if let Some(new_path) = new_hashes.get(&old_node.content_hash)
            && !consumed.contains(new_path)
        {
            renames.push((old_path.clone(), new_path.clone()));
            consumed.insert(new_path.clone());
        }
    }
    renames
}

fn sha256_hex(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    format!("{:x}", hasher.finalize())
}
