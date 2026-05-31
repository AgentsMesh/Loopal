use std::fs;
use tempfile::TempDir;

use loopal_memory::{
    Event, EventKind, EventLogWriter, RecallSource, RecallStats, RecallStatsMap, fold_events,
};

fn event(sid: &str, ts: i64, kind: EventKind) -> Event {
    Event::new(sid, ts, kind)
}

#[test]
fn append_single_then_fold_round_trip() {
    let tmp = TempDir::new().unwrap();
    let writer = EventLogWriter::new(tmp.path().to_path_buf(), "sess-aaaa1111");
    writer.append(EventKind::RecallHit {
        qid: "q1".into(),
        node: "foo".into(),
        rank: 0,
        score: 1.0,
        source: RecallSource::DirectHit,
    });
    let map = fold_events(tmp.path());
    let stats = map.get("foo").expect("foo stats");
    assert_eq!(stats.recall_count, 1);
    assert!(stats.last_recalled_at > 0);
}

#[test]
fn fold_multifile_sums_recall_count() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    let file_a = dir.join("2026-05-30_sessA.jsonl");
    let file_b = dir.join("2026-05-31_sessB.jsonl");
    let line = |sid: &str, ts: i64| {
        let ev = event(
            sid,
            ts,
            EventKind::RecallHit {
                qid: "q".into(),
                node: "n1".into(),
                rank: 0,
                score: 1.0,
                source: RecallSource::DirectHit,
            },
        );
        serde_json::to_string(&ev).unwrap()
    };
    fs::write(
        &file_a,
        format!("{}\n{}\n", line("sessA", 1000), line("sessA", 2000)),
    )
    .unwrap();
    fs::write(
        &file_b,
        format!("{}\n{}\n", line("sessB", 3000), line("sessB", 4000)),
    )
    .unwrap();

    let map = fold_events(dir);
    let stats = map.get("n1").unwrap();
    assert_eq!(stats.recall_count, 4);
    assert_eq!(stats.last_recalled_at, 4000);
}

#[test]
fn fold_skips_corrupt_lines() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("2026-05-30_aaa.jsonl");
    let good = serde_json::to_string(&event(
        "s",
        100,
        EventKind::RecallHit {
            qid: "q".into(),
            node: "foo".into(),
            rank: 0,
            score: 1.0,
            source: RecallSource::DirectHit,
        },
    ))
    .unwrap();
    fs::write(&path, format!("{}\nGARBAGE\n{}\n", good, good)).unwrap();

    let map = fold_events(tmp.path());
    let stats = map.get("foo").unwrap();
    assert_eq!(stats.recall_count, 2);
}

#[test]
fn fold_missing_dir_returns_empty() {
    let map = fold_events(std::path::Path::new("/nonexistent/path/xyz"));
    assert!(map.is_empty());
}

#[test]
fn importance_tag_latest_ts_wins() {
    let mut map = RecallStatsMap::new();
    let mut stats = RecallStats::default();

    let early = event(
        "s",
        100,
        EventKind::ImportanceTag {
            node: "foo".into(),
            importance: 1,
            tags: vec![],
            note: None,
        },
    );
    let late = event(
        "s",
        200,
        EventKind::ImportanceTag {
            node: "foo".into(),
            importance: 2,
            tags: vec![],
            note: None,
        },
    );

    stats.fold_event(&late);
    stats.fold_event(&early);
    map.insert("foo".into(), stats);

    let s = map.get("foo").unwrap();
    assert_eq!(s.importance, 2);
    assert_eq!(s.importance_ts, 200);
}

#[test]
fn two_sessions_independent_fold_sums() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    let w_a = EventLogWriter::new(dir.to_path_buf(), "sessAAAA");
    let w_b = EventLogWriter::new(dir.to_path_buf(), "sessBBBB");
    for _ in 0..3 {
        w_a.append(EventKind::RecallHit {
            qid: "q".into(),
            node: "shared".into(),
            rank: 0,
            score: 1.0,
            source: RecallSource::DirectHit,
        });
    }
    for _ in 0..2 {
        w_b.append(EventKind::RecallHit {
            qid: "q".into(),
            node: "shared".into(),
            rank: 0,
            score: 1.0,
            source: RecallSource::DirectHit,
        });
    }
    drop(w_a);
    drop(w_b);

    let map = fold_events(dir);
    assert_eq!(map.get("shared").unwrap().recall_count, 5);
}
