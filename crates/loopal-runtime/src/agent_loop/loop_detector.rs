use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use loopal_protocol::MessageSource;

use super::governance::traits::{Governance, Verdict};
use super::turn_context::TurnContext;

const WARN_THRESHOLD: u32 = 3;
const ABORT_THRESHOLD: u32 = 5;

/// Tracks tool call signatures and their cumulative occurrence count.
pub struct LoopDetector {
    /// (signature → cumulative count across the turn)
    signatures: HashMap<String, u32>,
    warn_threshold: u32,
    abort_threshold: u32,
}

impl Default for LoopDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl LoopDetector {
    pub fn new() -> Self {
        Self::with_thresholds(WARN_THRESHOLD, ABORT_THRESHOLD)
    }

    /// Create a detector with custom thresholds (from HarnessConfig).
    pub fn with_thresholds(warn: u32, abort: u32) -> Self {
        Self {
            signatures: HashMap::new(),
            warn_threshold: warn,
            abort_threshold: abort,
        }
    }
}

// Resets cumulative tool-call signatures on task boundaries. Delegates the
// predicate to `MessageSource::is_task_boundary` (hot path, no allocation);
// SSOT parity with `MessageOrigin::is_task_boundary` enforced by
// `loopal-protocol/tests/suite/task_boundary_test.rs`.
impl Governance for LoopDetector {
    fn on_before_tools(
        &mut self,
        _ctx: &mut TurnContext,
        tool_uses: &[(String, String, serde_json::Value)],
    ) -> Verdict {
        let mut worst = Verdict::Continue;

        for (_, name, input) in tool_uses {
            let sig = tool_signature(name, input);
            let count = self.signatures.entry(sig).or_insert(0);
            *count += 1;

            if *count >= self.abort_threshold {
                tracing::warn!(tool = name, count, "loop detected, aborting turn");
                return Verdict::AbortTurn {
                    reason: format!(
                        "Loop detected: tool '{name}' called {count} cumulative times \
                         with similar arguments. Aborting to prevent waste.",
                    ),
                    feedback_to_model: format!(
                        "Your previous `{name}` call was aborted by the runtime loop \
                         detector: the same call has been issued {count} times in this \
                         thread with no new information. Stop retrying with the same \
                         arguments. Either change strategy (different tool, different \
                         inputs, or ask the user for help) or report what you've learned \
                         so far and pause."
                    ),
                };
            }
            if *count >= self.warn_threshold {
                tracing::warn!(tool = name, count, "possible loop detected");
                worst = Verdict::InjectWarning(format!(
                    "[WARNING: Tool '{name}' has been called {count} times with similar \
                     arguments. You may be stuck in a loop. Try a different \
                     approach or ask the user for help.]",
                ));
            }
        }

        worst
    }

    fn on_envelope_received(&mut self, source: &MessageSource) {
        if source.is_task_boundary() {
            self.signatures.clear();
        }
    }
}

/// Build a stable signature from tool name + full input JSON.
///
/// We hash the **entire** serialized JSON (not a byte prefix). Prefix-based
/// hashing collided when distinguishing fields sorted late in the JSON
/// (e.g. `to` in `SendMessage`) were pushed past the cutoff by long
/// earlier fields — causing legitimate fan-out calls to be flagged as
/// loops. `serde_json::Value::Object` uses `BTreeMap`, so full-JSON
/// serialization is deterministic across equivalent inputs.
fn tool_signature(name: &str, input: &serde_json::Value) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    input.to_string().hash(&mut hasher);
    format!("{name}|{:x}", hasher.finish())
}
