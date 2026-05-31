use std::fs;
use std::path::Path;
use std::sync::Arc;

use tempfile::TempDir;
use tokio::runtime::Runtime;

use loopal_memory::graph::recall::{self, RecallParams};
use loopal_memory::{EventKind, EventLogWriter, MemoryGraph, fold_events, scan_directory};

fn write_note(dir: &Path, slug: &str, body: &str) {
    fs::write(dir.join(format!("{}.md", slug)), body).unwrap();
}

fn seed_fixture(dir: &Path) {
    write_note(
        dir,
        "topic-a",
        "---\nname: Topic A\ntype: project\n---\n\nA points to [[topic-b]].",
    );
    write_note(
        dir,
        "topic-b",
        "---\nname: Topic B\ntype: project\n---\n\nB points to [[topic-c]].",
    );
    write_note(
        dir,
        "topic-c",
        "---\nname: Topic C\ntype: project\n---\n\nC standalone leaf.",
    );
    write_note(
        dir,
        "unrelated",
        "---\nname: Unrelated\ntype: project\n---\n\nNothing about topics.",
    );
}

async fn build_graph(memory_dir: &Path, events_dir: &Path, sid: &str) -> MemoryGraph {
    let mut graph = MemoryGraph::in_memory().unwrap();
    fs::create_dir_all(events_dir).unwrap();
    let stats = fold_events(events_dir);
    graph.install_recall_stats(stats);
    graph.set_event_log(Arc::new(EventLogWriter::new(events_dir.to_path_buf(), sid)));
    scan_directory(&graph, memory_dir).await.unwrap();
    graph
}

#[test]
fn associative_recall_persists_reinforcement_across_sessions() {
    let memory_tmp = TempDir::new().unwrap();
    let events_tmp = TempDir::new().unwrap();
    let memory_dir = memory_tmp.path();
    let events_dir = events_tmp.path();
    seed_fixture(memory_dir);

    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let graph = build_graph(memory_dir, events_dir, "sessAAAA").await;

        let params = RecallParams {
            anchor_names: vec!["topic-a".into()],
            depth: 1,
            ..Default::default()
        };
        let r = recall::recall(&graph, &params).await.unwrap();
        let neighbor_ids: Vec<&str> = r.neighbors.iter().map(|n| n.node.id.as_str()).collect();
        assert!(neighbor_ids.contains(&"topic-b"));
        assert!(!neighbor_ids.contains(&"unrelated"));

        let reinforce = RecallParams {
            anchor_names: vec!["topic-b".into()],
            depth: 0,
            ..Default::default()
        };
        for _ in 0..5 {
            recall::recall(&graph, &reinforce).await.unwrap();
        }

        let in_session = graph.recall_stats_snapshot("topic-b").unwrap();
        assert!(in_session.recall_count >= 5);
    });

    let folded = fold_events(events_dir);
    let persisted = folded.get("topic-b").unwrap();
    assert!(persisted.recall_count >= 5);
    let unrelated_count = folded.get("unrelated").map(|s| s.recall_count).unwrap_or(0);
    assert_eq!(unrelated_count, 0);

    rt.block_on(async {
        let graph_b = build_graph(memory_dir, events_dir, "sessBBBB").await;
        let resumed = graph_b.recall_stats_snapshot("topic-b").unwrap();
        assert!(resumed.recall_count >= 5);

        let depth2 = RecallParams {
            anchor_names: vec!["topic-a".into()],
            depth: 2,
            ..Default::default()
        };
        let r2 = recall::recall(&graph_b, &depth2).await.unwrap();
        let by_id: std::collections::HashMap<&str, f32> = r2
            .neighbors
            .iter()
            .map(|n| (n.node.id.as_str(), n.score))
            .collect();
        let score_b = by_id.get("topic-b").copied().unwrap();
        let score_c = by_id.get("topic-c").copied().unwrap();
        assert!(score_b > score_c, "b={} c={}", score_b, score_c);
    });
}

#[test]
fn importance_ranking_lift() {
    let memory_tmp = TempDir::new().unwrap();
    let events_tmp = TempDir::new().unwrap();
    let memory_dir = memory_tmp.path();
    let events_dir = events_tmp.path();
    seed_fixture(memory_dir);

    let rt = Runtime::new().unwrap();

    let cold_b_score = rt.block_on(async {
        let graph = build_graph(memory_dir, events_dir, "sessCOLD").await;
        let params = RecallParams {
            anchor_names: vec!["topic-a".into()],
            depth: 2,
            ..Default::default()
        };
        let r = recall::recall(&graph, &params).await.unwrap();
        r.neighbors
            .iter()
            .find(|n| n.node.id == "topic-b")
            .map(|n| n.score)
            .expect("topic-b must appear at depth=2 from topic-a")
    });

    let writer = EventLogWriter::new(events_dir.to_path_buf(), "sessTAGB");
    writer.append(EventKind::ImportanceTag {
        node: "topic-b".into(),
        importance: 5,
        tags: vec!["critical".into()],
        note: None,
    });
    drop(writer);

    let warm_b_score = rt.block_on(async {
        let graph = build_graph(memory_dir, events_dir, "sessWARM").await;
        let snap = graph
            .recall_stats_snapshot("topic-b")
            .expect("ImportanceTag must survive into fold result");
        assert_eq!(snap.importance, 5);

        let params = RecallParams {
            anchor_names: vec!["topic-a".into()],
            depth: 2,
            ..Default::default()
        };
        let r = recall::recall(&graph, &params).await.unwrap();
        r.neighbors
            .iter()
            .find(|n| n.node.id == "topic-b")
            .map(|n| n.score)
            .expect("topic-b must still appear after tagging")
    });

    assert!(
        warm_b_score > cold_b_score,
        "importance must lift score: cold={} warm={}",
        cold_b_score,
        warm_b_score
    );
}
