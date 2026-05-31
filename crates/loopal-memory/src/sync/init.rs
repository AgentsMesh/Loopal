use std::collections::HashSet;
use std::path::Path;
use std::time::SystemTime;

use chrono::Utc;
use loopal_error::MemoryGraphError;
use walkdir::WalkDir;

use crate::extract::extract_file;
use crate::store::MemoryGraph;
use crate::store::types::{MemoryEdge, MemoryNode};
use crate::sync::{is_indexable_md, persist_extraction, relative_path};
use crate::synthesize;

pub struct InitStats {
    pub files_scanned: usize,
    pub files_skipped: usize,
    pub nodes_indexed: usize,
    pub edges_indexed: usize,
    pub errors: Vec<(String, String)>,
}

pub async fn scan_directory(
    graph: &MemoryGraph,
    dir: &Path,
) -> Result<InitStats, MemoryGraphError> {
    let mut stats = InitStats {
        files_scanned: 0,
        files_skipped: 0,
        nodes_indexed: 0,
        edges_indexed: 0,
        errors: Vec::new(),
    };

    if !dir.exists() {
        return Ok(stats);
    }

    let mut nodes_to_insert: Vec<MemoryNode> = Vec::new();
    let mut edges_to_insert: Vec<MemoryEdge> = Vec::new();
    let mut indexed_now: Vec<(String, String, i64, i64, i64)> = Vec::new();
    let now = Utc::now().timestamp_millis();

    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() || !is_indexable_md(path) {
            continue;
        }

        let rel = relative_path(dir, path);
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(e) => {
                stats.errors.push((rel, e.to_string()));
                continue;
            }
        };
        let size = meta.len() as i64;
        let modified_at = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        if let Ok(Some(cache)) = graph.get_file_cache(&rel).await
            && cache.modified_at == modified_at
            && cache.size == size
        {
            stats.files_skipped += 1;
            continue;
        }

        stats.files_scanned += 1;
        match tokio::fs::read_to_string(path).await {
            Ok(content) => {
                let result = extract_file(&rel, &content);
                if let Some(node) = result.nodes.first() {
                    indexed_now.push((
                        rel.clone(),
                        node.content_hash.clone(),
                        size,
                        modified_at,
                        now,
                    ));
                }
                nodes_to_insert.extend(result.nodes);
                edges_to_insert.extend(result.edges);
                for err in result.errors {
                    stats.errors.push((rel.clone(), err.to_string()));
                }
            }
            Err(e) => {
                stats.errors.push((rel, e.to_string()));
            }
        }
    }

    let known_ids: HashSet<String> = nodes_to_insert.iter().map(|n| n.id.clone()).collect();
    let batch = crate::extract::ExtractionResult {
        nodes: nodes_to_insert,
        edges: edges_to_insert,
        unresolved: Vec::new(),
        errors: Vec::new(),
    };
    let p = persist_extraction(graph, batch, None, &known_ids).await?;
    stats.nodes_indexed = p.nodes_indexed;
    stats.edges_indexed = p.edges_indexed;

    for (path, hash, size, modified_at, indexed_at) in indexed_now {
        let _ = graph
            .upsert_file_cache(&path, &hash, size, modified_at, indexed_at)
            .await;
    }

    if stats.nodes_indexed > 0 {
        synthesize::run_all(graph).await?;
    }
    Ok(stats)
}
