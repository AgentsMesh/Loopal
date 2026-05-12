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

/// File naming: `loopal-{YYYYMMDD-HHMMSS}-{pid}.log`; rotation segments
/// append `.1`, `.2`, etc. before `.log`.
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
        // reason: open O_RDWR (not O_WRONLY) so the fd survives a subsequent
        // `cleanup_old_logs` unlink: even when the path disappears, /dev/fd/N
        // still resolves and external tools can re-open the inode read-side.
        let file = OpenOptions::new()
            .create(true)
            .read(true)
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
            s.file = OpenOptions::new()
                .create(true)
                .read(true)
                .append(true)
                .open(path)?;
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

pub fn cleanup_old_logs(log_dir: &Path) {
    cleanup_with_alive_filter(log_dir, crate::bootstrap::is_alive);
}

// reason: a long-lived hub fd's path can be unlinked by a short-lived child's
// cleanup run, stranding KB of logs in an unreadable write-only fd; never
// delete a .log whose filename PID is still alive.
pub(crate) fn cleanup_with_alive_filter(log_dir: &Path, is_alive_fn: impl Fn(u32) -> bool) {
    let Ok(entries) = fs::read_dir(log_dir) else {
        return;
    };
    let mut logs: Vec<(PathBuf, u64, std::time::SystemTime, bool)> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            let s = e.file_name().to_string_lossy().into_owned();
            s.starts_with(LOG_PREFIX) && s.ends_with(LOG_EXT)
        })
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            let alive =
                pid_from_log_filename(&e.file_name().to_string_lossy()).is_some_and(&is_alive_fn);
            Some((e.path(), meta.len(), meta.modified().ok()?, alive))
        })
        .collect();
    logs.sort_by_key(|(_, _, t, _)| *t);
    let (total_count, total_size) = (logs.len(), logs.iter().map(|(_, s, _, _)| s).sum::<u64>());
    let (mut removed_count, mut removed_size) = (0usize, 0u64);
    for (path, size, _, alive) in &logs {
        if total_count - removed_count <= MAX_LOG_FILES && total_size - removed_size <= MAX_DIR_SIZE
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

pub(crate) fn pid_from_log_filename(name: &str) -> Option<u32> {
    let stem = name.strip_prefix(LOG_PREFIX)?.strip_suffix(LOG_EXT)?;
    let head = stem.rsplit_once('.').map(|(h, _)| h).unwrap_or(stem);
    let (_, pid) = head.rsplit_once('-')?;
    pid.parse::<u32>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Seek, SeekFrom};

    fn scoped_dir() -> PathBuf {
        let p = std::env::temp_dir().join(format!("loopal_lw_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn pid_from_log_filename_handles_known_shapes() {
        assert_eq!(
            pid_from_log_filename("loopal-20260512-105154-45582.log"),
            Some(45582)
        );
        assert_eq!(
            pid_from_log_filename("loopal-20260512-105154-45582.3.log"),
            Some(45582)
        );
        assert_eq!(pid_from_log_filename("sandbox_test"), None);
        assert_eq!(pid_from_log_filename("loopal-no-pid.log"), None);
    }

    #[test]
    fn cleanup_never_deletes_alive_pid_log() {
        let dir = scoped_dir();
        let alive = dir.join("loopal-20260512-100000-1001.log");
        fs::write(&alive, b"x").unwrap();
        for i in 0..MAX_LOG_FILES + 2 {
            fs::write(dir.join(format!("loopal-20260512-110000-{i}.log")), b"x").unwrap();
        }
        cleanup_with_alive_filter(&dir, |pid| pid == 1001);
        assert!(alive.exists(), "alive PID's log must survive cleanup");
        let kept = fs::read_dir(&dir).unwrap().count();
        assert!(
            kept <= MAX_LOG_FILES + 1,
            "expected ≤20 dead + 1 alive, got {kept}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn new_writer_fd_supports_read() {
        let dir = scoped_dir();
        let mut w = RotatingFileWriter::new(&dir);
        w.write_all(b"abc").unwrap();
        w.flush().unwrap();
        let mut clone = w.state.file.try_clone().unwrap();
        clone.seek(SeekFrom::Start(0)).unwrap();
        let mut buf = String::new();
        clone.read_to_string(&mut buf).unwrap();
        assert_eq!(buf, "abc");
        let _ = fs::remove_dir_all(&dir);
    }
}
