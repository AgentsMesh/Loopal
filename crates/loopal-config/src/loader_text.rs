use std::path::Path;

// reason: cap config-injected text at 100KB so an oversized or binary file
// can't blow up the agent's context window or OOM the loader.
const MAX_TEXT_BYTES: u64 = 100 * 1024;

pub(crate) fn read_optional_text(path: &Path) -> Option<String> {
    read_text_bounded(path, MAX_TEXT_BYTES)
}

pub(crate) fn read_text_bounded(path: &Path, max_bytes: u64) -> Option<String> {
    if !path.exists() {
        return None;
    }
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() > max_bytes {
        tracing::warn!(
            path = %path.display(),
            bytes = meta.len(),
            limit = max_bytes,
            "skipping oversize text file"
        );
        return None;
    }
    match std::fs::read_to_string(path) {
        Ok(s) => Some(s),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "text file read failed (non-UTF8?)");
            None
        }
    }
}

pub(crate) const TEXT_BYTE_LIMIT: u64 = MAX_TEXT_BYTES;
