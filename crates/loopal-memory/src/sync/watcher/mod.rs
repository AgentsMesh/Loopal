mod rename;
mod setup;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use loopal_error::MemoryGraphError;
use tokio::sync::mpsc;
use tracing::warn;

use crate::extract::{extract_file, slug_from_path};
use crate::render::render_memory_md;
use crate::store::MemoryGraph;
use crate::sync::{is_indexable_md, memory_index_path, persist_extraction, relative_path};
use crate::synthesize;

const SYNTH_THROTTLE: Duration = Duration::from_secs(5);

pub use setup::{WatcherHandle, watch};

pub(crate) async fn process_events(
    graph: Arc<MemoryGraph>,
    base: PathBuf,
    mut rx: mpsc::UnboundedReceiver<Vec<PathBuf>>,
) {
    let mut last_synth = Instant::now()
        .checked_sub(SYNTH_THROTTLE)
        .unwrap_or_else(Instant::now);
    while let Some(paths) = rx.recv().await {
        let any_change = handle_batch(&graph, &base, &paths).await;
        if !any_change {
            continue;
        }
        if last_synth.elapsed() >= SYNTH_THROTTLE {
            if let Err(e) = synthesize::run_all(&graph).await {
                warn!(error = %e, "watcher: synthesize batch failed");
            }
            last_synth = Instant::now();
        }
        rerender_index(&graph, &base).await;
    }
}

async fn handle_batch(graph: &MemoryGraph, base: &Path, paths: &[PathBuf]) -> bool {
    let (deletes, updates) = classify_paths(base, paths).await;
    let renames = rename::detect(graph, base, &deletes, &updates).await;

    let renamed_old: HashSet<&PathBuf> = renames.iter().map(|(d, _)| d).collect();
    let renamed_new: HashSet<&PathBuf> = renames.iter().map(|(_, u)| u).collect();

    let mut any_change = false;

    let batch_slugs: HashSet<String> = updates
        .iter()
        .filter(|p| !renamed_new.contains(p))
        .map(|p| slug_from_path(&relative_path(base, p)))
        .collect();

    for (old_path, new_path) in &renames {
        match handle_rename(graph, base, old_path, new_path).await {
            Ok(true) => any_change = true,
            Ok(false) => {
                if let Ok(true) = handle_delete(graph, base, old_path).await {
                    any_change = true;
                }
                if let Ok(true) = handle_update(graph, base, new_path, &batch_slugs).await {
                    any_change = true;
                }
            }
            Err(e) => warn!(error = %e, path = %new_path.display(), "watcher: rename failed"),
        }
    }

    for path in &deletes {
        if renamed_old.contains(path) {
            continue;
        }
        match handle_delete(graph, base, path).await {
            Ok(true) => any_change = true,
            Ok(false) => {}
            Err(e) => warn!(error = %e, path = %path.display(), "watcher: delete failed"),
        }
    }

    for path in &updates {
        if renamed_new.contains(path) {
            continue;
        }
        match handle_update(graph, base, path, &batch_slugs).await {
            Ok(true) => any_change = true,
            Ok(false) => {}
            Err(e) => warn!(error = %e, path = %path.display(), "watcher: update failed"),
        }
    }

    any_change
}

async fn classify_paths(base: &Path, paths: &[PathBuf]) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut deletes = Vec::new();
    let mut updates = Vec::new();
    for path in paths {
        if is_memory_index_path(base, path) {
            continue;
        }
        if !is_indexable_md(path) {
            continue;
        }
        if path.exists() {
            updates.push(path.clone());
        } else {
            deletes.push(path.clone());
        }
    }
    (deletes, updates)
}

async fn handle_rename(
    graph: &MemoryGraph,
    base: &Path,
    old_path: &Path,
    new_path: &Path,
) -> Result<bool, MemoryGraphError> {
    let old_rel = relative_path(base, old_path);
    let new_rel = relative_path(base, new_path);
    let old_slug = slug_from_path(&old_rel);
    let new_slug = slug_from_path(&new_rel);
    graph.rename_node(&old_slug, &new_slug, &new_rel).await
}

async fn handle_delete(
    graph: &MemoryGraph,
    base: &Path,
    path: &Path,
) -> Result<bool, MemoryGraphError> {
    let rel = relative_path(base, path);
    let slug = slug_from_path(&rel);
    let _ = graph.delete_file_cache(&rel).await;
    graph.delete_node(&slug).await
}

async fn handle_update(
    graph: &MemoryGraph,
    base: &Path,
    path: &Path,
    batch_known: &HashSet<String>,
) -> Result<bool, MemoryGraphError> {
    let rel = relative_path(base, path);
    let content = match tokio::fs::read_to_string(path).await {
        Ok(c) => c,
        Err(e) => {
            warn!(error = %e, path = %rel, "watcher: read failed");
            return Ok(false);
        }
    };
    let result = extract_file(&rel, &content);
    let slug = result.nodes.first().map(|n| n.id.clone());
    persist_extraction(graph, result, slug.as_deref(), batch_known).await?;
    Ok(true)
}

fn is_memory_index_path(base: &Path, path: &Path) -> bool {
    memory_index_path(base) == path
}

async fn rerender_index(graph: &MemoryGraph, base: &Path) {
    let md = match render_memory_md(graph).await {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "watcher: render_memory_md failed");
            return;
        }
    };
    let path = memory_index_path(base);
    if let Err(e) = tokio::fs::write(&path, md).await {
        warn!(error = %e, path = %path.display(), "watcher: write MEMORY.md failed");
    }
}
