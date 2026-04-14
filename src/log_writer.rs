use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Max size for a single log file: 50 MB.
const MAX_FILE_SIZE: u64 = 50 * 1024 * 1024;
/// Max total size of the logs directory: 200 MB.
const MAX_DIR_SIZE: u64 = 200 * 1024 * 1024;
/// Max number of log files to retain (includes rotation segments).
const MAX_LOG_FILES: usize = 20;
/// Prefix for log file names.
const LOG_PREFIX: &str = "loopal-";
/// Extension for log file names.
const LOG_EXT: &str = ".log";

/// A file writer that creates a per-run log file and rotates when it exceeds
/// [`MAX_FILE_SIZE`].
///
/// File naming: `loopal-{YYYYMMDD-HHMMSS}-{pid}.log`
/// On rotation:  `loopal-{YYYYMMDD-HHMMSS}-{pid}.1.log`, `.2.log`, etc.
pub struct RotatingFileWriter {
    state: WriterState,
}

struct WriterState {
    file: File,
    written: u64,
    dir: PathBuf,
    base_stem: String,
    seq: u32,
}

impl RotatingFileWriter {
    pub fn new(log_dir: &Path) -> Self {
        let now = chrono::Local::now();
        let pid = std::process::id();
        let base_stem = format!("{LOG_PREFIX}{}-{pid}", now.format("%Y%m%d-%H%M%S"),);
        let path = log_dir.join(format!("{base_stem}{LOG_EXT}"));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .unwrap_or_else(|_| File::open("/dev/null").expect("/dev/null must be openable"));

        Self {
            state: WriterState {
                file,
                written: 0,
                dir: log_dir.to_path_buf(),
                base_stem,
                seq: 0,
            },
        }
    }

    /// Returns the path of the current (initial) log file.
    pub fn current_path(&self) -> String {
        let s = &self.state;
        let name = if s.seq == 0 {
            format!("{}{LOG_EXT}", s.base_stem)
        } else {
            format!("{}.{}{LOG_EXT}", s.base_stem, s.seq)
        };
        s.dir.join(name).to_string_lossy().into_owned()
    }
}

impl Write for RotatingFileWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let s = &mut self.state;
        // Rotate if current file exceeds size limit
        if s.written >= MAX_FILE_SIZE {
            s.seq += 1;
            let name = format!("{}.{}{LOG_EXT}", s.base_stem, s.seq);
            let path = s.dir.join(name);
            s.file = OpenOptions::new().create(true).append(true).open(path)?;
            s.written = 0;
        }
        let n = s.file.write(buf)?;
        s.written += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.state.file.flush()
    }
}

/// Remove old log files to keep the directory within retention limits.
///
/// Strategy: sort by modification time (oldest first), remove until both
/// file count ≤ [`MAX_LOG_FILES`] and total size ≤ [`MAX_DIR_SIZE`].
pub fn cleanup_old_logs(log_dir: &Path) {
    let Ok(entries) = fs::read_dir(log_dir) else {
        return;
    };

    let mut logs: Vec<(PathBuf, u64, std::time::SystemTime)> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name();
            let s = name.to_string_lossy();
            s.starts_with(LOG_PREFIX) && s.ends_with(LOG_EXT)
        })
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            Some((e.path(), meta.len(), meta.modified().ok()?))
        })
        .collect();

    // Sort oldest first
    logs.sort_by_key(|(_, _, t)| *t);

    let total_count = logs.len();
    let total_size: u64 = logs.iter().map(|(_, s, _)| s).sum();

    let mut removed_size = 0u64;

    for (removed_count, (path, size, _)) in logs.iter().enumerate() {
        let remaining_count = total_count - removed_count;
        let remaining_size = total_size - removed_size;
        if remaining_count <= MAX_LOG_FILES && remaining_size <= MAX_DIR_SIZE {
            break;
        }
        let _ = fs::remove_file(path);
        removed_size += size;
    }
}
