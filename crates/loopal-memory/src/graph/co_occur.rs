use std::collections::HashMap;

use loopal_error::MemoryGraphError;

use crate::graph::recall::{CoOccurringNode, RecallParams};
use crate::graph::score::co_occurrence_bonus;
use crate::store::MemoryGraph;
use crate::store::types::{EdgeKind, MemoryKind, MemoryNode, Provenance};

const DEFAULT_LIMIT: usize = 5;

pub(super) async fn collect(
    graph: &MemoryGraph,
    anchor_ids: &[String],
    params: &RecallParams,
) -> Result<Vec<CoOccurringNode>, MemoryGraphError> {
    if !params.include_synthesized {
        return Ok(Vec::new());
    }
    let anchor_set: std::collections::HashSet<&String> = anchor_ids.iter().collect();
    let mut by_target: HashMap<String, (f32, String)> = HashMap::new();

    for anchor in anchor_ids {
        let outgoing = graph.get_outgoing_edges(anchor).await?;
        let incoming = graph.get_incoming_edges(anchor).await?;
        for e in outgoing.into_iter().chain(incoming) {
            if e.provenance != Provenance::Synthesized
                || !matches!(e.kind, EdgeKind::CoOccursSlug | EdgeKind::CoOccursToken)
                || e.confidence < params.min_confidence
            {
                continue;
            }
            let other = if &e.src_id == anchor {
                e.dst_id.clone()
            } else {
                e.src_id.clone()
            };
            if anchor_set.contains(&other) {
                continue;
            }
            let weight = co_occurrence_bonus(&e);
            by_target
                .entry(other)
                .and_modify(|(w, _)| {
                    if weight > *w {
                        *w = weight;
                    }
                })
                .or_insert((weight, anchor.clone()));
        }
    }

    let ids: Vec<String> = by_target.keys().cloned().collect();
    let nodes = graph.get_nodes(&ids).await?;
    let mut node_map: HashMap<String, MemoryNode> = nodes
        .into_iter()
        .filter(|n| n.kind != MemoryKind::Index)
        .map(|n| (n.id.clone(), n))
        .collect();
    let mut out = Vec::with_capacity(by_target.len());
    for (target, (weight, via)) in by_target {
        if let Some(node) = node_map.remove(&target) {
            out.push(CoOccurringNode {
                node,
                weight,
                via_anchor: via,
            });
        }
    }
    out.sort_by(|a, b| {
        b.weight
            .partial_cmp(&a.weight)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out.truncate(DEFAULT_LIMIT);
    Ok(out)
}
