use std::fs;

use tempfile::TempDir;

use loopal_memory::{
    EventKind, EventLogWriter, RecallSource, ensure_gitignore, fold_events, run_gc,
};

#[test]
fn gc_compresses_old_jsonl() {
    let tmp = TempDir::new().unwrap();
    let writer = EventLogWriter::new(tmp.path().to_path_buf(), "sessAAAA");
    writer.append(EventKind::RecallHit {
        qid: "q".into(),
        node: "foo".into(),
        rank: 0,
        score: 1.0,
        source: RecallSource::DirectHit,
    });
    drop(writer);

    let original: Vec<_> = fs::read_dir(tmp.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    let target = original
        .iter()
        .find(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .map(|n| n.ends_with(".jsonl"))
                .unwrap_or(false)
        })
        .expect("found jsonl");
    let renamed = tmp.path().join("2024-01-15_sessAAAA.jsonl");
    fs::rename(target, &renamed).unwrap();

    let stats = run_gc(tmp.path(), 90, 365);
    assert_eq!(stats.compressed, 1);
    assert_eq!(stats.errors, 0);
    assert!(!renamed.exists());
    assert!(tmp.path().join("2024-01-15_sessAAAA.jsonl.gz").exists());
}

#[test]
fn fold_transparently_reads_gz() {
    let tmp = TempDir::new().unwrap();
    let writer = EventLogWriter::new(tmp.path().to_path_buf(), "sessBBBB");
    for _ in 0..3 {
        writer.append(EventKind::RecallHit {
            qid: "q".into(),
            node: "bar".into(),
            rank: 0,
            score: 1.0,
            source: RecallSource::DirectHit,
        });
    }
    drop(writer);

    let jsonl_path = fs::read_dir(tmp.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .map(|n| n.ends_with(".jsonl"))
                .unwrap_or(false)
        })
        .expect("jsonl exists");
    let renamed = tmp.path().join("2024-01-15_sessBBBB.jsonl");
    fs::rename(&jsonl_path, &renamed).unwrap();

    run_gc(tmp.path(), 90, 365);
    assert!(tmp.path().join("2024-01-15_sessBBBB.jsonl.gz").exists());

    let map = fold_events(tmp.path());
    assert_eq!(map.get("bar").unwrap().recall_count, 3);
}

#[test]
fn gc_archives_old_gz() {
    let tmp = TempDir::new().unwrap();
    let gz_path = tmp.path().join("2023-01-15_sessCCCC.jsonl.gz");
    fs::write(&gz_path, b"placeholder").unwrap();

    let stats = run_gc(tmp.path(), 90, 365);
    assert_eq!(stats.archived, 1);
    assert!(!gz_path.exists());
    assert!(
        tmp.path()
            .join("archive")
            .join("2023-01-15_sessCCCC.jsonl.gz")
            .exists()
    );
}

#[test]
fn gitignore_creates_when_missing() {
    let tmp = TempDir::new().unwrap();
    ensure_gitignore(tmp.path()).unwrap();
    let content = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
    assert!(content.contains(".loopal/memory-events/archive/"));
    assert!(content.contains(".loopal/memory-events/*.jsonl.gz"));
}

#[test]
fn gitignore_idempotent_no_duplicate() {
    let tmp = TempDir::new().unwrap();
    ensure_gitignore(tmp.path()).unwrap();
    ensure_gitignore(tmp.path()).unwrap();
    let content = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
    let count = content.matches(".loopal/memory-events/archive/").count();
    assert_eq!(count, 1);
}

#[test]
fn gitignore_preserves_existing_rules() {
    let tmp = TempDir::new().unwrap();
    let existing = "node_modules/\n.env\n";
    fs::write(tmp.path().join(".gitignore"), existing).unwrap();
    ensure_gitignore(tmp.path()).unwrap();
    let content = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
    assert!(content.contains("node_modules/"));
    assert!(content.contains(".env"));
    assert!(content.contains(".loopal/memory-events/archive/"));
}

#[test]
fn gitignore_handles_non_utf8_existing_file() {
    let tmp = TempDir::new().unwrap();
    let gitignore = tmp.path().join(".gitignore");
    let mut bytes = b".existing-rule\n".to_vec();
    bytes.extend_from_slice(&[0xFF, 0xFE, 0xFD]);
    bytes.extend_from_slice(b"\n");
    fs::write(&gitignore, &bytes).unwrap();

    ensure_gitignore(tmp.path()).unwrap();
    ensure_gitignore(tmp.path()).unwrap();

    let content = fs::read(&gitignore).unwrap();
    let lossy = String::from_utf8_lossy(&content);
    assert_eq!(
        lossy.matches(".loopal/memory-events/archive/").count(),
        1,
        "rules must not duplicate when existing file is non-UTF8"
    );
    assert!(lossy.contains(".existing-rule"));
}
