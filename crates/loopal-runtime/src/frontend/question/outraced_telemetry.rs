// Telemetry for "outraced" classifier decisions.
//
// **Domain term**: when a `ClassifierQuestionHandler` runs the classifier and
// the user in parallel, the **user can answer first**, in which case the
// classifier's would-be answer is discarded. We call this the classifier
// being "outraced" by the user (the classifier ran the race, but lost).
//
// We still record the classifier's discarded answer + the user's actual
// answer to a JSONL file so the agreement rate can be analysed offline.
//
// **No rotation by design**: this is best-effort observability, written at
// most once per AskUser race (so ≤1 row per user-facing question). Long-running
// sessions accumulate a few MB at most; users who care can rotate / truncate
// the file out-of-band.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::Serialize;
use tracing::warn;

use loopal_protocol::UserQuestionResponse;

#[derive(Serialize)]
pub(super) struct OutracedRecord<'a> {
    pub ts: String,
    pub question_id: &'a str,
    pub manual_answers: &'a [String],
    pub classifier_answers: &'a [String],
    pub manual_duration_ms: u64,
    pub classifier_duration_ms: u64,
    pub agreement: bool,
}

pub(super) struct OutracedInput<'a> {
    pub question_id: &'a str,
    pub manual_answers: &'a [String],
    pub classifier_answers: &'a [String],
    pub manual_duration_ms: u64,
    pub classifier_duration_ms: u64,
}

pub(super) fn extract_answers(response: &UserQuestionResponse) -> Vec<String> {
    match response {
        UserQuestionResponse::Answered { answers, .. } => answers.clone(),
        _ => Vec::new(),
    }
}

pub(super) fn record_outraced(input: OutracedInput<'_>) {
    let agreement = answers_equivalent(input.manual_answers, input.classifier_answers);
    let ts = chrono::Utc::now().to_rfc3339();
    append_record(OutracedRecord {
        ts,
        question_id: input.question_id,
        manual_answers: input.manual_answers,
        classifier_answers: input.classifier_answers,
        manual_duration_ms: input.manual_duration_ms,
        classifier_duration_ms: input.classifier_duration_ms,
        agreement,
    });
}

// reason: multi-select answers should agree regardless of label order;
// ["A","B"] and ["B","A"] represent the same user intent.
pub(super) fn answers_equivalent(a: &[String], b: &[String]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut a_sorted: Vec<&str> = a.iter().map(String::as_str).collect();
    let mut b_sorted: Vec<&str> = b.iter().map(String::as_str).collect();
    a_sorted.sort();
    b_sorted.sort();
    a_sorted == b_sorted
}

fn append_record(record: OutracedRecord<'_>) {
    let line = match serde_json::to_string(&record) {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "failed to serialize outraced telemetry record");
            return;
        }
    };
    let path = telemetry_path();
    if let Err(e) = ensure_dir(&path) {
        warn!(path = %path.display(), error = %e, "failed to create telemetry dir");
        return;
    }
    // reason: file lock prevents JSONL line corruption when multiple sessions
    // race to append concurrently. POSIX append is atomic for writes < PIPE_BUF
    // (~4KB) but JSONL rows can exceed that; an explicit mutex is the cheapest
    // guarantee.
    let lock = telemetry_lock().lock().unwrap_or_else(|e| e.into_inner());
    match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(mut f) => {
            if let Err(e) = writeln!(f, "{line}") {
                warn!(path = %path.display(), error = %e, "telemetry write failed");
            }
        }
        Err(e) => warn!(path = %path.display(), error = %e, "telemetry open failed"),
    }
    drop(lock);
}

fn telemetry_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn telemetry_path() -> PathBuf {
    if let Ok(p) = std::env::var("LOOPAL_TELEMETRY_DIR") {
        return PathBuf::from(p).join("classifier_outraced.jsonl");
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home)
        .join(".loopal")
        .join("telemetry")
        .join("classifier_outraced.jsonl")
}

fn ensure_dir(path: &std::path::Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}
