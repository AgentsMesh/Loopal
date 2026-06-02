use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use loopal_protocol::MessageSource;
use loopal_provider_api::ContentBlock;

use super::governance::traits::{Governance, Verdict};
use super::turn_context::TurnContext;

const WARN_THRESHOLD: u32 = 3;
const ABORT_THRESHOLD: u32 = 5;

// Tracks, per (tool, input), how many CONSECUTIVE times it produced the SAME
// output. A tool re-reading a mutating path (e.g. ReadImage on a screenshot
// path that is overwritten every call — same args, fresh pixels each time)
// keeps resetting the streak and is never flagged. Only stationary repetition
// (same input → same output) accrues toward warn/abort.
pub struct LoopDetector {
    repeats: HashMap<u64, Repeat>,
    warn_threshold: u32,
    abort_threshold: u32,
}

struct Repeat {
    output: u64,
    count: u32,
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

    pub fn with_thresholds(warn: u32, abort: u32) -> Self {
        Self {
            repeats: HashMap::new(),
            warn_threshold: warn,
            abort_threshold: abort,
        }
    }
}

impl Governance for LoopDetector {
    fn on_before_tools(
        &mut self,
        _ctx: &mut TurnContext,
        tool_uses: &[(String, String, serde_json::Value)],
    ) -> Verdict {
        let mut worst = Verdict::Continue;
        for (_, name, input) in tool_uses {
            let count = self
                .repeats
                .get(&input_signature(name, input))
                .map(|r| r.count)
                .unwrap_or(0);
            if count >= self.abort_threshold {
                tracing::warn!(tool = name, count, "loop detected, aborting turn");
                return Verdict::AbortTurn {
                    reason: format!(
                        "Loop detected: tool '{name}' produced identical output \
                         {count} consecutive times. Aborting to prevent waste.",
                    ),
                    feedback_to_model: format!(
                        "Your `{name}` call was aborted by the runtime loop detector: \
                         the same call returned identical output {count} times in a row \
                         with no new information. Stop retrying with the same arguments. \
                         Change strategy (different tool, different inputs, or ask the \
                         user) or report what you've learned and pause."
                    ),
                };
            }
            if count >= self.warn_threshold {
                tracing::warn!(tool = name, count, "possible loop detected");
                worst = Verdict::InjectWarning(format!(
                    "[WARNING: Tool '{name}' returned identical output {count} times in a \
                     row. You may be stuck in a loop. Try a different approach or ask the \
                     user for help.]",
                ));
            }
        }
        worst
    }

    // Counting happens here (post-execution), not in on_before_tools, because
    // the streak keys on OUTPUT. Consequence: if a tool batch aborts with a
    // turn-level Err before this runs (execute_tools returns Err in
    // turn_tool_phase), that batch is not counted — such error-loops are bounded
    // by try_recover's retry caps / transition_error instead of by this detector.
    fn on_after_tools(
        &mut self,
        _ctx: &mut TurnContext,
        tool_uses: &[(String, String, serde_json::Value)],
        results: &[ContentBlock],
    ) {
        let mut counted = std::collections::HashSet::new();
        for (id, name, input) in tool_uses {
            let Some(out) = output_digest_for(results, id) else {
                continue;
            };
            let sig = input_signature(name, input);
            if !counted.insert(sig) {
                continue;
            }
            match self.repeats.get_mut(&sig) {
                Some(r) if r.output == out => r.count += 1,
                _ => {
                    self.repeats.insert(
                        sig,
                        Repeat {
                            output: out,
                            count: 1,
                        },
                    );
                }
            }
        }
    }

    fn on_envelope_received(&mut self, source: &MessageSource) {
        if source.is_task_boundary() {
            self.repeats.clear();
        }
    }

    // Compaction discards earlier turns: the calls that fed our counter are no
    // longer in the store, so the streak is measuring history that no longer
    // exists. Reset to avoid carrying pre-compact counts forward.
    fn on_compact_completed(&mut self) {
        self.repeats.clear();
    }

    // A user interrupt resets the loop streak: counts accrued before the
    // cancellation must not span it into the next turn.
    fn on_turn_cancelled(&mut self) {
        self.repeats.clear();
    }
}

fn input_signature(name: &str, input: &serde_json::Value) -> u64 {
    let mut h = DefaultHasher::new();
    name.hash(&mut h);
    // serde_json::Value::Object is a BTreeMap → deterministic serialization.
    input.to_string().hash(&mut h);
    h.finish()
}

// Digest the tool_result paired (by id) with this call: content + error flag
// + image identity + metadata. Returns None when no matching result is present,
// so the caller skips the streak update (an absent result is NOT "identical
// output" — folding all missing results to one empty hash would falsely accrue
// loops).
fn output_digest_for(results: &[ContentBlock], id: &str) -> Option<u64> {
    for b in results {
        if let ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
            images,
            metadata,
        } = b
            && tool_use_id == id
        {
            let mut h = DefaultHasher::new();
            content.hash(&mut h);
            is_error.hash(&mut h);
            for img in images {
                img.media_type().hash(&mut h);
                img.content_key().hash(&mut h);
            }
            // Metadata distinguishes results with identical content (e.g. a
            // Write returning a fixed banner but a changing byte count).
            if let Some(md) = metadata
                && let Ok(s) = serde_json::to_string(md)
            {
                s.hash(&mut h);
            }
            return Some(h.finish());
        }
    }
    None
}
