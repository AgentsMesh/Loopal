use loopal_memory::extract::extract_file;
use loopal_memory::graph::recall::{RecallParams, RecallResult, recall};
use loopal_memory::{EventLogWriter, MemoryGraph, synthesize_all};

use crate::baseline::{grep_filter_baseline, read_all_baseline, recall_output_tokens};
use crate::fixture::{Fixture, all as all_fixtures, memory_index};
use crate::ground_truth::{QueryMode, QuerySpec, load};
use crate::metrics::{f1_at_k, mrr, ndcg_at_k, precision_at_k, recall_at_k};
use crate::synth_eval;

#[allow(dead_code)]
pub struct QueryReport {
    pub spec: QuerySpec,
    pub retrieved: Vec<String>,
    pub p_at_5: f32,
    pub p_at_10: f32,
    pub r_at_5: f32,
    pub r_at_10: f32,
    pub f1_at_5: f32,
    pub mrr: f32,
    pub ndcg_at_5: f32,
    pub recall_tokens: usize,
    pub grep_tokens: usize,
    pub read_all_tokens: usize,
}

pub struct SynthReport {
    pub synthesizer: String,
    pub sampled: usize,
    pub plausible: usize,
}

pub struct EvalReport {
    pub queries: Vec<QueryReport>,
    pub synth: Vec<SynthReport>,
    pub fixture_count: usize,
    pub total_bytes: usize,
}

pub async fn run_with_warmup(warmup_reps: usize) -> EvalReport {
    run_internal(warmup_reps, 0, false).await
}

pub async fn run_with_importance(importance: i8) -> EvalReport {
    run_internal(0, importance, false).await
}

pub async fn run_with_per_query_warmup(warmup_reps: usize) -> EvalReport {
    run_internal(warmup_reps, 0, true).await
}

async fn run_internal(warmup_reps: usize, importance: i8, per_query: bool) -> EvalReport {
    let mut graph = MemoryGraph::in_memory().expect("open in-memory graph");
    let fixtures = all_fixtures();
    let total_bytes: usize = fixtures.iter().map(|f| f.bytes()).sum();

    let memory_idx = memory_index();
    extract_and_load(&graph, &memory_idx).await;
    for f in &fixtures {
        extract_and_load(&graph, f).await;
    }

    synthesize_all(&graph).await.expect("synthesize");

    let gt = load();

    if warmup_reps > 0 && !per_query {
        let tmp = tempfile::TempDir::new().expect("tmp events dir");
        let writer =
            std::sync::Arc::new(EventLogWriter::new(tmp.path().to_path_buf(), "eval-warmup"));
        graph.set_event_log(writer);
        for spec in &gt.queries {
            for rel in &spec.relevant {
                let warm_params = RecallParams {
                    anchor_names: vec![rel.id.clone()],
                    depth: 0,
                    ..Default::default()
                };
                for _ in 0..warmup_reps {
                    let _ = recall(&graph, &warm_params).await;
                }
            }
        }
    }

    if warmup_reps > 0 && per_query {
        let tmp = tempfile::TempDir::new().expect("tmp events dir");
        let writer =
            std::sync::Arc::new(EventLogWriter::new(tmp.path().to_path_buf(), "eval-per-q"));
        graph.set_event_log(writer);
    }

    if importance != 0 {
        let mut stats_map = loopal_memory::RecallStatsMap::new();
        for spec in &gt.queries {
            for rel in &spec.relevant {
                stats_map.insert(
                    rel.id.clone(),
                    loopal_memory::RecallStats {
                        recall_count: 0,
                        last_recalled_at: 0,
                        importance,
                        importance_ts: 1,
                    },
                );
            }
        }
        graph.install_recall_stats(stats_map);
    }

    let mut query_reports = Vec::with_capacity(gt.queries.len());
    for spec in gt.queries {
        if warmup_reps > 0 && per_query {
            graph.install_recall_stats(loopal_memory::RecallStatsMap::new());
            for rel in &spec.relevant {
                let warm_params = RecallParams {
                    anchor_names: vec![rel.id.clone()],
                    depth: 0,
                    ..Default::default()
                };
                for _ in 0..warmup_reps {
                    let _ = recall(&graph, &warm_params).await;
                }
            }
        }
        let report = run_one_query(&graph, &fixtures, spec).await;
        query_reports.push(report);
    }

    let synth = synth_eval::score(&graph, &query_reports).await;

    EvalReport {
        queries: query_reports,
        synth,
        fixture_count: fixtures.len() + 1,
        total_bytes,
    }
}

async fn extract_and_load(graph: &MemoryGraph, fixture: &Fixture) {
    let result = extract_file(fixture.file_path, fixture.content);
    for node in result.nodes {
        let _ = graph.upsert_node(node).await;
    }
    for edge in result.edges {
        let _ = graph.insert_edge(edge).await;
    }
}

async fn run_one_query(graph: &MemoryGraph, fixtures: &[Fixture], spec: QuerySpec) -> QueryReport {
    let params = match spec.mode {
        QueryMode::Query => RecallParams {
            query: spec.query.clone(),
            depth: 2,
            ..Default::default()
        },
        QueryMode::Anchor => RecallParams {
            anchor_names: spec.anchors.clone(),
            depth: 2,
            ..Default::default()
        },
        QueryMode::Mixed => RecallParams {
            query: spec.query.clone(),
            anchor_names: spec.anchors.clone(),
            depth: 2,
            ..Default::default()
        },
    };
    let result = recall(graph, &params).await.expect("recall");
    let retrieved_raw = retrieved_ids(&result);
    // anchor 是用户输入种子，不是检索系统的"预测"。算 precision/recall 时排除自身，
    // 否则 q06/q07/q08 这类 anchor mode 永远在 rank 1 拉低 P@K（anchor ∉ relevant 是 ground truth 约定）。
    let anchor_set: std::collections::HashSet<&str> =
        spec.anchors.iter().map(|s| s.as_str()).collect();
    let retrieved: Vec<String> = retrieved_raw
        .into_iter()
        .filter(|id| !anchor_set.contains(id.as_str()))
        .collect();
    let formatted = loopal_memory::format_recall(&result);

    let rel_set = spec.relevant_ids();
    let rel_map = spec.relevance_map();
    let query_str = spec.query.clone().unwrap_or_else(|| spec.anchors.join(" "));

    QueryReport {
        p_at_5: precision_at_k(&rel_set, &retrieved, 5),
        p_at_10: precision_at_k(&rel_set, &retrieved, 10),
        r_at_5: recall_at_k(&rel_set, &retrieved, 5),
        r_at_10: recall_at_k(&rel_set, &retrieved, 10),
        f1_at_5: f1_at_k(&rel_set, &retrieved, 5),
        mrr: mrr(&rel_set, &retrieved),
        ndcg_at_5: ndcg_at_k(&rel_map, &retrieved, 5),
        recall_tokens: recall_output_tokens(&formatted),
        grep_tokens: grep_filter_baseline(fixtures, &query_str),
        read_all_tokens: read_all_baseline(fixtures),
        spec,
        retrieved,
    }
}

fn retrieved_ids(result: &RecallResult) -> Vec<String> {
    let mut ids = Vec::new();
    for hit in &result.direct_hits {
        ids.push(hit.node.id.clone());
    }
    for n in &result.neighbors {
        if !ids.contains(&n.node.id) {
            ids.push(n.node.id.clone());
        }
    }
    for c in &result.co_occurring {
        if !ids.contains(&c.node.id) {
            ids.push(c.node.id.clone());
        }
    }
    ids
}
