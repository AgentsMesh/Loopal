pub mod derive_chain;
pub mod slug_cluster;
pub mod supersede;
pub mod tfidf;
pub mod token_cluster;

use chrono::Utc;

use loopal_error::MemoryGraphError;

use crate::store::MemoryGraph;
use crate::store::queries_edge;
use crate::store::types::Provenance;

pub struct SynthesisStats {
    pub edges_added: usize,
    pub edges_removed: usize,
    pub by_synthesizer: Vec<(String, usize)>,
}

pub async fn run_all(graph: &MemoryGraph) -> Result<SynthesisStats, MemoryGraphError> {
    let now = Utc::now().timestamp_millis();

    graph
        .db
        .with_conn_mut(move |conn| {
            let tx = conn.transaction()?;

            let removed = queries_edge::delete_by_provenance(&tx, Provenance::Synthesized)?;
            let slug = slug_cluster::synthesize_sync(&tx, now)?;
            let derive = derive_chain::synthesize_sync(&tx, now)?;
            let sup = supersede::synthesize_sync(&tx, now)?;
            let tokens = token_cluster::synthesize_sync(&tx, now)?;

            tx.commit()?;

            Ok(SynthesisStats {
                edges_added: slug + derive + sup + tokens,
                edges_removed: removed,
                by_synthesizer: vec![
                    ("slug_cluster".into(), slug),
                    ("derive_chain".into(), derive),
                    ("supersede".into(), sup),
                    ("token_cluster".into(), tokens),
                ],
            })
        })
        .await
}
