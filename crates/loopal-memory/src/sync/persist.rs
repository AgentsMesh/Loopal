use std::collections::HashSet;
use std::path::Path;

use loopal_error::MemoryGraphError;

use crate::extract::ExtractionResult;
use crate::store::MemoryGraph;
use crate::store::queries_edge;
use crate::store::queries_node;

pub struct PersistStats {
    pub nodes_indexed: usize,
    pub edges_indexed: usize,
}

pub fn relative_path(base: &Path, path: &Path) -> String {
    match path.strip_prefix(base) {
        Ok(rel) => rel.to_string_lossy().into_owned(),
        Err(_) => {
            tracing::warn!(
                base = %base.display(),
                path = %path.display(),
                "relative_path: path is not under base — caller must canonicalize both"
            );
            path.to_string_lossy().into_owned()
        }
    }
}

pub async fn persist_extraction(
    graph: &MemoryGraph,
    result: ExtractionResult,
    clean_outgoing_for: Option<&str>,
    extra_known_ids: &HashSet<String>,
) -> Result<PersistStats, MemoryGraphError> {
    let clean_owned = clean_outgoing_for.map(|s| s.to_string());
    let extra_owned = extra_known_ids.clone();

    graph
        .db
        .with_conn_mut(move |conn| {
            let tx = conn.transaction()?;
            let mut stats = PersistStats {
                nodes_indexed: 0,
                edges_indexed: 0,
            };

            if let Some(slug) = clean_owned {
                queries_edge::delete_outgoing(&tx, &slug)?;
            }

            let mut known_ids: HashSet<String> = extra_owned;
            for n in &result.nodes {
                known_ids.insert(n.id.clone());
            }

            for node in result.nodes {
                queries_node::upsert(&tx, &node)?;
                stats.nodes_indexed += 1;
            }
            for edge in result.edges {
                if !known_ids.contains(&edge.dst_id)
                    && queries_node::get(&tx, &edge.dst_id)?.is_none()
                {
                    continue;
                }
                if !known_ids.contains(&edge.src_id)
                    && queries_node::get(&tx, &edge.src_id)?.is_none()
                {
                    continue;
                }
                queries_edge::insert(&tx, &edge)?;
                stats.edges_indexed += 1;
            }

            tx.commit()?;
            Ok(stats)
        })
        .await
}
