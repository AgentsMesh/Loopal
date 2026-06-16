use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const MAX_FILES_PER_KIND: usize = 100;
const MAX_BYTES_PER_KIND: u64 = 50 * 1024 * 1024;
const KIND_PREFIXES: [&str; 2] = ["traces-", "metrics-"];
const EXT: &str = ".jsonl";

/// Bound the JSONL telemetry directory. Every process leaves a
/// `traces-{ts}-{pid}.jsonl` + `metrics-{ts}-{pid}.jsonl` pair, so without a cap
/// the directory grows monotonically across the multiprocess fleet. Enforces a
/// per-kind file-count and byte-size ceiling, dropping the oldest first.
///
/// `is_alive` guards a long-lived process's open file from being unlinked by a
/// short-lived child's cleanup run (same hazard as log rotation). Only files
/// matching the `traces-`/`metrics-` prefixes are ever touched — sibling files
/// like `secret_access.jsonl` are left alone.
pub fn cleanup_telemetry_files(dir: &Path, is_alive: impl Fn(u32) -> bool) {
    for prefix in KIND_PREFIXES {
        enforce_kind(dir, prefix, &is_alive);
    }
}

fn enforce_kind(dir: &Path, prefix: &str, is_alive: &impl Fn(u32) -> bool) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(PathBuf, u64, SystemTime, bool)> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.starts_with(prefix) && name.ends_with(EXT)
        })
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            let alive =
                pid_from_telemetry_filename(&e.file_name().to_string_lossy()).is_some_and(is_alive);
            Some((e.path(), meta.len(), meta.modified().ok()?, alive))
        })
        .collect();
    files.sort_by_key(|(_, _, t, _)| *t);
    let total_count = files.len();
    let total_size: u64 = files.iter().map(|(_, s, _, _)| s).sum();
    let (mut removed_count, mut removed_size) = (0usize, 0u64);
    for (path, size, _, alive) in &files {
        if total_count - removed_count <= MAX_FILES_PER_KIND
            && total_size - removed_size <= MAX_BYTES_PER_KIND
        {
            break;
        }
        if *alive {
            continue;
        }
        let _ = fs::remove_file(path);
        removed_count += 1;
        removed_size += size;
    }
}

pub(crate) fn pid_from_telemetry_filename(name: &str) -> Option<u32> {
    let stem = KIND_PREFIXES
        .iter()
        .find_map(|p| name.strip_prefix(p))?
        .strip_suffix(EXT)?;
    let (_, pid) = stem.rsplit_once('-')?;
    pid.parse::<u32>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scoped_dir() -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("loopal_tele_{}_{n}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn pid_parsing_handles_both_kinds() {
        assert_eq!(
            pid_from_telemetry_filename("traces-20260410-012952-64885.jsonl"),
            Some(64885)
        );
        assert_eq!(
            pid_from_telemetry_filename("metrics-20260410-012952-64885.jsonl"),
            Some(64885)
        );
        assert_eq!(pid_from_telemetry_filename("secret_access.jsonl"), None);
        assert_eq!(pid_from_telemetry_filename("traces-nopid.txt"), None);
    }

    #[test]
    fn never_deletes_alive_pid_file() {
        let dir = scoped_dir();
        let alive = dir.join("traces-20260512-100000-1001.jsonl");
        fs::write(&alive, b"x").unwrap();
        for i in 0..MAX_FILES_PER_KIND + 5 {
            fs::write(dir.join(format!("traces-20260512-110000-{i}.jsonl")), b"x").unwrap();
        }
        cleanup_telemetry_files(&dir, |pid| pid == 1001);
        assert!(alive.exists(), "alive PID's file must survive cleanup");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn enforces_per_kind_file_count() {
        let dir = scoped_dir();
        for i in 0..MAX_FILES_PER_KIND + 30 {
            fs::write(dir.join(format!("metrics-20260512-110000-{i}.jsonl")), b"x").unwrap();
        }
        cleanup_telemetry_files(&dir, |_| false);
        let kept = fs::read_dir(&dir).unwrap().count();
        assert!(kept <= MAX_FILES_PER_KIND, "expected ≤cap, got {kept}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn leaves_unrelated_files_untouched() {
        let dir = scoped_dir();
        let secret = dir.join("secret_access.jsonl");
        let outraced = dir.join("classifier_outraced.jsonl");
        fs::write(&secret, b"s").unwrap();
        fs::write(&outraced, b"o").unwrap();
        for i in 0..MAX_FILES_PER_KIND + 10 {
            fs::write(dir.join(format!("traces-20260512-110000-{i}.jsonl")), b"x").unwrap();
        }
        cleanup_telemetry_files(&dir, |_| false);
        assert!(secret.exists(), "secret_access.jsonl must never be touched");
        assert!(outraced.exists(), "unrelated telemetry must be left alone");
        let _ = fs::remove_dir_all(&dir);
    }
}
