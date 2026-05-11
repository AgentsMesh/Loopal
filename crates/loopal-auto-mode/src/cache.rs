use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use loopal_tool_api::PermissionDecision;

use crate::ClassifierResult;

const CACHE_TTL_SECS: u64 = 60;
const CACHE_MAX_ENTRIES: usize = 128;

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

    pub fn get(&self, tool_name: &str, input: &serde_json::Value) -> Option<ClassifierResult> {
        let key = make_key(tool_name, input);
        let mut map = self.inner.lock().unwrap();
        let entry = map.get(&key)?;
        if entry.created.elapsed().as_secs() > CACHE_TTL_SECS {
            map.remove(&key);
            return None;
        }
        Some(ClassifierResult::ok(entry.decision, entry.reason.clone()))
    }

    pub fn put(&self, tool_name: &str, input: &serde_json::Value, result: &ClassifierResult) {
        if result.error.is_some() {
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
