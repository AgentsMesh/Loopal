use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use loopal_tool_api::PermissionDecision;

use crate::ClassifierResult;

/// Time-to-live for cached classification results.
const CACHE_TTL_SECS: u64 = 60;
/// Maximum number of cached entries.
const CACHE_MAX_ENTRIES: usize = 128;

/// Simple time-bounded cache for classifier decisions.
///
/// Key: `(tool_name, serialized_input)` for exact match (no hash collisions).
/// Entries expire after `CACHE_TTL_SECS` and are evicted on access.
pub(crate) struct ClassifierCache {
    inner: Mutex<HashMap<CacheKey, CacheEntry>>,
}

type CacheKey = (String, String);

struct CacheEntry {
    decision: PermissionDecision,
    reason: String,
    created: Instant,
}

impl ClassifierCache {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    /// Look up a cached decision. Returns `None` if not found or expired.
    pub fn get(&self, tool_name: &str, input: &serde_json::Value) -> Option<ClassifierResult> {
        let key = make_key(tool_name, input);
        let mut map = self.inner.lock().unwrap();
        let entry = map.get(&key)?;
        if entry.created.elapsed().as_secs() > CACHE_TTL_SECS {
            map.remove(&key);
            return None;
        }
        Some(ClassifierResult {
            decision: entry.decision,
            reason: entry.reason.clone(),
            duration_ms: 0,
        })
    }

    /// Store a classification result. Evicts oldest entries if over capacity.
    pub fn put(&self, tool_name: &str, input: &serde_json::Value, result: &ClassifierResult) {
        // Only cache definitive decisions, not errors that defaulted to Deny.
        if result.reason.starts_with("Classifier error:") || result.reason.contains("parse failure")
        {
            return;
        }
        let key = make_key(tool_name, input);
        let mut map = self.inner.lock().unwrap();
        if map.len() >= CACHE_MAX_ENTRIES {
            evict_oldest(&mut map);
        }
        map.insert(
            key,
            CacheEntry {
                decision: result.decision,
                reason: result.reason.clone(),
                created: Instant::now(),
            },
        );
    }
}

fn make_key(tool_name: &str, input: &serde_json::Value) -> CacheKey {
    (tool_name.to_string(), input.to_string())
}

fn evict_oldest(map: &mut HashMap<CacheKey, CacheEntry>) {
    if let Some(oldest_key) = map
        .iter()
        .min_by_key(|(_, e)| e.created)
        .map(|(k, _)| k.clone())
    {
        map.remove(&oldest_key);
    }
}
