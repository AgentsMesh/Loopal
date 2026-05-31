use super::{RecallParams, RecallResult};
use crate::event_log::{EventKind, RecallSource};
use crate::store::MemoryGraph;

pub(super) fn emit_events(graph: &MemoryGraph, params: &RecallParams, result: &RecallResult) {
    let qid = format!(
        "q-{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    );
    let result_count =
        result.direct_hits.len() + result.neighbors.len() + result.co_occurring.len();
    graph.record_event(EventKind::QueryEvent {
        qid: qid.clone(),
        query: params.query.clone(),
        anchor: params.anchor_names.clone(),
        result_count: result_count as u32,
        latency_ms: 0,
        caller: Some("recall".into()),
    });
    for (rank, hit) in result.direct_hits.iter().enumerate() {
        graph.record_event(EventKind::RecallHit {
            qid: qid.clone(),
            node: hit.node.id.clone(),
            rank: rank as u32,
            score: hit.score,
            source: RecallSource::DirectHit,
        });
    }
    for (rank, n) in result.neighbors.iter().enumerate() {
        graph.record_event(EventKind::RecallHit {
            qid: qid.clone(),
            node: n.node.id.clone(),
            rank: (result.direct_hits.len() + rank) as u32,
            score: n.score,
            source: RecallSource::Neighbor,
        });
    }
    for (rank, c) in result.co_occurring.iter().enumerate() {
        graph.record_event(EventKind::RecallHit {
            qid: qid.clone(),
            node: c.node.id.clone(),
            rank: (result.direct_hits.len() + result.neighbors.len() + rank) as u32,
            score: c.weight,
            source: RecallSource::CoOccur,
        });
    }
}
