use loopal_memory::synthesize::run_all;
use loopal_memory::synthesize::tfidf::{
    cosine_similarity, document_frequency, term_frequency, tf_idf_vector, tokenize,
};
use loopal_memory::{EdgeKind, MemoryEdge, MemoryGraph, MemoryKind, MemoryNode, Provenance};

fn node(id: &str, body: &str) -> MemoryNode {
    MemoryNode {
        id: id.into(),
        kind: MemoryKind::Project,
        name: id.into(),
        description: None,
        file_path: format!("{}.md", id),
        body_preview: body.into(),
        created_at: 1,
        updated_at: 1,
        ttl_days: None,
        content_hash: "h".repeat(64),
        indexed_at: 1,
    }
}

async fn token_edges(g: &MemoryGraph) -> Vec<MemoryEdge> {
    g.list_edges_by_provenance(Provenance::Synthesized)
        .await
        .unwrap()
        .into_iter()
        .filter(|e| e.kind == EdgeKind::CoOccursToken)
        .collect()
}

#[test]
fn tokenize_lowercases_and_filters_stopwords() {
    let toks = tokenize("The Quick Brown Fox jumps OVER");
    assert!(toks.contains(&"quick".to_string()));
    assert!(toks.contains(&"brown".to_string()));
    assert!(!toks.contains(&"the".to_string()));
    assert!(!toks.contains(&"over".to_string()));
}

#[test]
fn tokenize_drops_single_char_tokens() {
    let toks = tokenize("a b c hello world");
    assert!(!toks.contains(&"a".to_string()));
    assert_eq!(toks.len(), 2);
}

#[test]
fn tokenize_handles_chinese() {
    let toks = tokenize("速率限制和冷却时间");
    assert!(!toks.is_empty());
    assert!(!toks.contains(&"的".to_string()));
    assert!(!toks.contains(&"和".to_string()));
}

#[test]
fn cosine_similarity_identical_vectors_yields_one() {
    let a = term_frequency(&["alpha".into(), "beta".into(), "alpha".into()]);
    let sim = cosine_similarity(&a, &a);
    assert!((sim - 1.0).abs() < 1e-5);
}

#[test]
fn cosine_similarity_disjoint_vectors_yields_zero() {
    let a = term_frequency(&["alpha".into(), "beta".into()]);
    let b = term_frequency(&["gamma".into(), "delta".into()]);
    let sim = cosine_similarity(&a, &b);
    assert_eq!(sim, 0.0);
}

#[test]
fn document_frequency_counts_each_term_once_per_doc() {
    let docs = vec![
        vec!["alpha".into(), "alpha".into(), "beta".into()],
        vec!["alpha".into(), "gamma".into()],
    ];
    let df = document_frequency(&docs);
    assert_eq!(df.get("alpha"), Some(&2));
    assert_eq!(df.get("beta"), Some(&1));
}

#[test]
fn tf_idf_rare_term_outweighs_common_term() {
    let docs = vec![
        vec!["common".into(), "common".into(), "rare".into()],
        vec!["common".into(), "other".into()],
        vec!["common".into(), "another".into()],
    ];
    let df = document_frequency(&docs);
    let tf0 = term_frequency(&docs[0]);
    let vec0 = tf_idf_vector(&tf0, &df, docs.len());
    assert!(vec0["rare"] > vec0["common"]);
}

#[tokio::test]
async fn token_cluster_connects_similar_bodies() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node("rate", "rate limit cooldown automation policy"))
        .await
        .unwrap();
    g.upsert_node(node(
        "cooldown",
        "cooldown rate limit automation enforcement",
    ))
    .await
    .unwrap();
    g.upsert_node(node(
        "unrelated",
        "completely different topic banana fruit smoothie",
    ))
    .await
    .unwrap();

    run_all(&g).await.unwrap();

    let synth_edges = token_edges(&g).await;
    let has_pair = synth_edges.iter().any(|e| {
        (e.src_id == "rate" && e.dst_id == "cooldown")
            || (e.src_id == "cooldown" && e.dst_id == "rate")
    });
    assert!(has_pair, "expected token pair, got {:?}", synth_edges);
}

#[tokio::test]
async fn token_cluster_skips_below_threshold() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node("a", "alpha beta gamma delta"))
        .await
        .unwrap();
    g.upsert_node(node("b", "epsilon zeta eta theta iota"))
        .await
        .unwrap();
    run_all(&g).await.unwrap();
    assert!(token_edges(&g).await.is_empty());
}

#[tokio::test]
async fn token_cluster_with_under_two_nodes_does_nothing() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node("solo", "alone")).await.unwrap();
    run_all(&g).await.unwrap();
    assert!(token_edges(&g).await.is_empty());
}
