use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Duration, NaiveDate, Utc};
use flate2::Compression;
use flate2::write::GzEncoder;

const ARCHIVE_DIR: &str = "archive";

pub struct GcStats {
    pub compressed: usize,
    pub archived: usize,
    pub errors: usize,
}

pub fn run_gc(dir: &Path, compress_after_days: u32, archive_after_days: u32) -> GcStats {
    let mut stats = GcStats {
        compressed: 0,
        archived: 0,
        errors: 0,
    };
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return stats,
        Err(e) => {
            tracing::warn!(error = %e, path = %dir.display(), "event_log gc read_dir failed");
            stats.errors = 1;
            return stats;
        }
    };
    let today = Utc::now().date_naive();
    let compress_cutoff = today - Duration::days(compress_after_days as i64);
    let archive_cutoff = today - Duration::days(archive_after_days as i64);

    let archive_dir = dir.join(ARCHIVE_DIR);
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(date) = parse_event_file_date(&path) else {
            continue;
        };
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");

        if name.ends_with(".jsonl.gz") && date < archive_cutoff {
            tally(
                &mut stats,
                archive_file(&path, &archive_dir),
                "archive",
                &path,
            );
        } else if name.ends_with(".jsonl") && date < compress_cutoff {
            tally(&mut stats, compress_file(&path), "compress", &path);
        }
    }
    stats
}

fn tally(stats: &mut GcStats, result: io::Result<bool>, op: &'static str, path: &Path) {
    match result {
        Ok(true) if op == "archive" => stats.archived += 1,
        Ok(true) => stats.compressed += 1,
        Ok(false) => {}
        Err(e) => {
            stats.errors += 1;
            tracing::warn!(error = %e, op, path = %path.display(), "event_log gc op failed");
        }
    }
}

fn parse_event_file_date(path: &Path) -> Option<NaiveDate> {
    let name = path.file_name().and_then(|s| s.to_str())?;
    let stem = name
        .strip_suffix(".jsonl.gz")
        .or_else(|| name.strip_suffix(".jsonl"))?;
    let date_part = stem.split('_').next()?;
    NaiveDate::parse_from_str(date_part, "%Y-%m-%d").ok()
}

fn compress_file(path: &Path) -> io::Result<bool> {
    let gz_path = with_gz_suffix(path);
    if gz_path.exists() {
        // Orphan recovery: prior compress crashed between rename and remove,
        // or a concurrent compress just won the rename race. The .gz is the
        // authoritative copy; drop the stale source to prevent fold double-count.
        fs::remove_file(path)?;
        tracing::warn!(
            path = %path.display(),
            "event_log gc: orphan .jsonl removed (gz already exists)"
        );
        return Ok(false);
    }
    let tmp_path = unique_tmp_path(&gz_path);
    {
        let src = File::open(path)?;
        let tmp = File::create(&tmp_path)?;
        let mut encoder = GzEncoder::new(BufWriter::new(tmp), Compression::default());
        io::copy(&mut BufReader::new(src), &mut encoder)?;
        encoder.finish()?.flush()?;
    }
    if let Err(e) = fs::rename(&tmp_path, &gz_path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(e);
    }
    fs::remove_file(path)?;
    Ok(true)
}

fn archive_file(path: &Path, archive_dir: &Path) -> io::Result<bool> {
    fs::create_dir_all(archive_dir)?;
    let Some(name) = path.file_name() else {
        return Ok(false);
    };
    let dest = archive_dir.join(name);
    if dest.exists() {
        fs::remove_file(path)?;
        return Ok(false);
    }
    fs::rename(path, &dest)?;
    Ok(true)
}

fn with_gz_suffix(path: &Path) -> PathBuf {
    let mut buf = path.as_os_str().to_owned();
    buf.push(".gz");
    PathBuf::from(buf)
}

fn unique_tmp_path(gz_path: &Path) -> PathBuf {
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let mut buf = gz_path.as_os_str().to_owned();
    buf.push(format!(".tmp.{pid}.{nanos}"));
    PathBuf::from(buf)
}
