use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;

/// Append-only JSONL sink that creates its backing file on the first write.
/// A process that emits no spans/metrics never touches the filesystem, so
/// short-lived processes (sub-agents, CLI subcommands, a hub child that exits
/// early) stop leaving empty `traces-`/`metrics-` files behind.
pub(crate) struct LazyJsonlFile {
    path: PathBuf,
    writer: Option<BufWriter<File>>,
}

impl LazyJsonlFile {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { path, writer: None }
    }

    pub(crate) fn write_line(&mut self, line: &str) {
        if let Some(w) = self.writer_mut() {
            let _ = writeln!(w, "{line}");
        }
    }

    pub(crate) fn flush(&mut self) {
        if let Some(w) = self.writer.as_mut() {
            let _ = w.flush();
        }
    }

    fn writer_mut(&mut self) -> Option<&mut BufWriter<File>> {
        if self.writer.is_none() {
            if let Some(parent) = self.path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
                .ok()?;
            self.writer = Some(BufWriter::new(file));
        }
        self.writer.as_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scoped_path(tag: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("loopal_lazy_{}_{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join(format!("{tag}.jsonl"))
    }

    #[test]
    fn no_file_created_without_write() {
        let path = scoped_path("traces-x-1");
        let sink = LazyJsonlFile::new(path.clone());
        drop(sink);
        assert!(!path.exists(), "file must not exist until first write");
    }

    #[test]
    fn flush_without_write_creates_nothing() {
        let path = scoped_path("metrics-x-1");
        let mut sink = LazyJsonlFile::new(path.clone());
        sink.flush();
        assert!(!path.exists(), "flush alone must not create the file");
    }

    #[test]
    fn first_write_creates_and_appends() {
        let path = scoped_path("traces-x-2");
        let mut sink = LazyJsonlFile::new(path.clone());
        sink.write_line("{\"a\":1}");
        sink.write_line("{\"b\":2}");
        sink.flush();
        let body = std::fs::read_to_string(&path).unwrap();
        assert_eq!(body, "{\"a\":1}\n{\"b\":2}\n");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
