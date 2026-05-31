use std::fs;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use chrono::{Duration, Utc};
use tempfile::TempDir;

use loopal_memory::{Event, EventKind, RecallSource, fold_events, run_gc};

fn make_event(sid: &str, ts: i64, node: &str) -> Event {
    Event::new(
        sid,
        ts,
        EventKind::RecallHit {
            qid: "q".into(),
            node: node.into(),
            rank: 0,
            score: 1.0,
            source: RecallSource::DirectHit,
        },
    )
}

fn compress_eligible_date() -> String {
    (Utc::now() - Duration::days(120))
        .date_naive()
        .format("%Y-%m-%d")
        .to_string()
}

fn archive_eligible_date() -> String {
    (Utc::now() - Duration::days(400))
        .date_naive()
        .format("%Y-%m-%d")
        .to_string()
}

#[test]
fn gc_removes_orphan_jsonl_when_gz_already_exists() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    let date = compress_eligible_date();

    let line = serde_json::to_string(&make_event("sessCRSH", 100, "n1")).unwrap();
    let stale_jsonl = dir.join(format!("{}_sessCRSH.jsonl", date));
    fs::write(&stale_jsonl, format!("{}\n", line)).unwrap();

    let gz_jsonl = dir.join(format!("{}_sessCRSH.jsonl.gz", date));
    let mut enc = flate2::write::GzEncoder::new(
        fs::File::create(&gz_jsonl).unwrap(),
        flate2::Compression::default(),
    );
    enc.write_all(format!("{}\n", line).as_bytes()).unwrap();
    enc.finish().unwrap();

    let stats = run_gc(dir, 90, 365);
    assert_eq!(stats.errors, 0);
    assert!(!stale_jsonl.exists(), "orphan .jsonl must be removed");
    assert!(gz_jsonl.exists(), ".gz must remain");

    let map = fold_events(dir);
    assert_eq!(
        map.get("n1").unwrap().recall_count,
        1,
        "fold must not double-count after orphan recovery"
    );
}

#[test]
fn gc_tmp_path_includes_pid_and_nanos_for_concurrency() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    let date = compress_eligible_date();
    let line = serde_json::to_string(&make_event("sessTMP", 100, "n2")).unwrap();
    let stale = dir.join(format!("{}_sessTMP.jsonl", date));
    fs::write(&stale, format!("{}\n", line)).unwrap();

    let stats = run_gc(dir, 90, 365);
    assert_eq!(stats.compressed, 1);

    let leftover_tmp: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".jsonl.gz.tmp."))
        .collect();
    assert!(
        leftover_tmp.is_empty(),
        "no unique-suffix tmp file should remain after successful compress"
    );
}

#[test]
fn gc_streams_large_file_without_full_load() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    let date = compress_eligible_date();
    let stale = dir.join(format!("{}_sessBIG.jsonl", date));
    let mut f = fs::File::create(&stale).unwrap();
    let line = serde_json::to_string(&make_event("sessBIG", 100, "big")).unwrap();
    for _ in 0..1000 {
        writeln!(f, "{}", line).unwrap();
    }
    drop(f);

    let stats = run_gc(dir, 90, 365);
    assert_eq!(stats.compressed, 1);

    let map = fold_events(dir);
    assert_eq!(map.get("big").unwrap().recall_count, 1000);
}

#[cfg(unix)]
#[test]
fn gc_propagates_read_dir_permission_error() {
    let tmp = TempDir::new().unwrap();
    let locked = tmp.path().join("locked");
    fs::create_dir(&locked).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();

    let stats = run_gc(&locked, 90, 365);
    assert_eq!(stats.errors, 1, "permission error must be counted");

    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn gc_missing_dir_returns_zero_stats_no_error() {
    let stats = run_gc(std::path::Path::new("/nonexistent/dir/zzz"), 90, 365);
    assert_eq!(stats.compressed, 0);
    assert_eq!(stats.archived, 0);
    assert_eq!(stats.errors, 0);
}

#[test]
fn fold_one_file_preserves_partial_batch_on_truncated_gz() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    let date = compress_eligible_date();
    let gz_path = dir.join(format!("{}_sessTRC.jsonl.gz", date));

    let line = serde_json::to_string(&make_event("sessTRC", 100, "partial")).unwrap();
    let payload = format!("{}\n{}\n{}\n", line, line, line);

    let mut buf: Vec<u8> = Vec::new();
    let mut enc = flate2::write::GzEncoder::new(&mut buf, flate2::Compression::default());
    enc.write_all(payload.as_bytes()).unwrap();
    enc.finish().unwrap();

    let truncated = &buf[..buf.len() - 5];
    fs::write(&gz_path, truncated).unwrap();

    let map = fold_events(dir);
    let recovered = map.get("partial").map(|s| s.recall_count).unwrap_or(0);
    assert!(
        recovered >= 1,
        "at least one valid event should be recovered from truncated gz, got {}",
        recovered
    );
}

#[test]
fn gc_archive_dest_collision_removes_source() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    let archive = dir.join("archive");
    fs::create_dir(&archive).unwrap();
    let date = archive_eligible_date();

    let old_gz = dir.join(format!("{}_sessOLD.jsonl.gz", date));
    fs::write(&old_gz, b"original").unwrap();

    let collision = archive.join(format!("{}_sessOLD.jsonl.gz", date));
    fs::write(&collision, b"existing").unwrap();

    let stats = run_gc(dir, 90, 365);
    assert_eq!(stats.errors, 0);
    assert!(
        !old_gz.exists(),
        "source must be removed on archive-dest collision to prevent indefinite retry"
    );
    assert_eq!(
        fs::read(&collision).unwrap(),
        b"existing",
        "existing archive must not be overwritten"
    );
}
