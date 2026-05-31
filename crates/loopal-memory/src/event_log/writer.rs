use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{NaiveDate, Utc};
use flate2::read::GzDecoder;

use crate::event_log::recall_stats::{RecallStatsMap, apply_events_to_map};
use crate::event_log::schema::{Event, EventKind};

pub struct EventLogWriter {
    dir: PathBuf,
    today: Mutex<Option<(NaiveDate, String, File)>>,
    sid: String,
}

impl EventLogWriter {
    pub fn new(dir: impl Into<PathBuf>, session_id: impl Into<String>) -> Self {
        Self {
            dir: dir.into(),
            today: Mutex::new(None),
            sid: session_id.into(),
        }
    }

    pub fn append(&self, kind: EventKind) -> Event {
        let now = Utc::now();
        let ts = now.timestamp_millis();
        let date = now.date_naive();
        let ev = Event::new(self.sid.clone(), ts, kind);
        if let Err(e) = self.write_line(date, &ev) {
            tracing::warn!(error = %e, "event_log append failed");
        }
        ev
    }

    fn write_line(&self, date: NaiveDate, ev: &Event) -> std::io::Result<()> {
        let sid_short = short_sid(&self.sid);
        let path = self
            .dir
            .join(format!("{}_{}.jsonl", date.format("%Y-%m-%d"), sid_short));

        let mut guard = self
            .today
            .lock()
            .map_err(|e| std::io::Error::other(format!("event_log mutex poisoned: {e}")))?;
        let need_reopen = match guard.as_ref() {
            Some((d, p, _)) => *d != date || p != path.to_string_lossy().as_ref(),
            None => true,
        };
        if need_reopen {
            std::fs::create_dir_all(&self.dir)?;
            let file = OpenOptions::new().create(true).append(true).open(&path)?;
            *guard = Some((date, path.to_string_lossy().into_owned(), file));
        }
        let (_, _, file) = guard.as_mut().expect("event_log file just opened");
        let mut line = serde_json::to_vec(ev).map_err(std::io::Error::other)?;
        line.push(b'\n');
        file.write_all(&line)?;
        Ok(())
    }
}

fn short_sid(sid: &str) -> String {
    sid.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(8)
        .collect()
}

pub fn fold_events(dir: &Path) -> RecallStatsMap {
    let mut map = RecallStatsMap::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return map;
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| is_event_file(p))
        .collect();
    files.sort();
    for path in files {
        if let Err(e) = fold_one_file(&path, &mut map) {
            tracing::warn!(error = %e, path = %path.display(), "event_log fold file failed");
        }
    }
    map
}

fn is_event_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    name.ends_with(".jsonl") || name.ends_with(".jsonl.gz")
}

fn open_reader(path: &Path) -> std::io::Result<Box<dyn Read>> {
    let file = File::open(path)?;
    let is_gz = path
        .file_name()
        .and_then(|s| s.to_str())
        .map(|n| n.ends_with(".gz"))
        .unwrap_or(false);
    if is_gz {
        Ok(Box::new(GzDecoder::new(file)))
    } else {
        Ok(Box::new(file))
    }
}

fn fold_one_file(path: &Path, map: &mut RecallStatsMap) -> std::io::Result<()> {
    let reader = BufReader::new(open_reader(path)?);
    let mut batch: Vec<Event> = Vec::new();
    let mut io_err: Option<std::io::Error> = None;
    for (idx, line) in reader.lines().enumerate() {
        match line {
            Ok(l) => {
                if l.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<Event>(&l) {
                    Ok(ev) => batch.push(ev),
                    Err(e) => tracing::warn!(
                        line = idx + 1,
                        path = %path.display(),
                        error = %e,
                        "event_log skip corrupt line"
                    ),
                }
            }
            Err(e) => {
                tracing::warn!(
                    line = idx + 1,
                    path = %path.display(),
                    error = %e,
                    "event_log fold io error mid-file; applying partial batch"
                );
                io_err = Some(e);
                break;
            }
        }
    }
    apply_events_to_map(map, &batch);
    match io_err {
        Some(e) => Err(e),
        None => Ok(()),
    }
}
