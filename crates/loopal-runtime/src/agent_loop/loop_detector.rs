//! Detects repeated tool call patterns across LLM iterations.
//!
//! When the same tool is called with identical arguments multiple times,
//! the agent is likely stuck in a loop. This observer warns after
//! `WARN_THRESHOLD` cumulative repeats and aborts after `ABORT_THRESHOLD`.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use super::turn_context::TurnContext;
use super::turn_observer::{ObserverAction, TurnObserver};

const WARN_THRESHOLD: u32 = 3;
const ABORT_THRESHOLD: u32 = 5;
/// Max bytes of input JSON used for signature (avoids hashing huge payloads).
const SIGNATURE_INPUT_LIMIT: usize = 200;

/// Tracks tool call signatures and their cumulative occurrence count.
#[derive(Default)]
pub struct LoopDetector {
    /// (signature → cumulative count across the turn)
    signatures: HashMap<String, u32>,
}

impl LoopDetector {
    pub fn new() -> Self {
        Self::default()
    }
}

impl TurnObserver for LoopDetector {
    fn on_before_tools(
        &mut self,
        _ctx: &mut TurnContext,
        tool_uses: &[(String, String, serde_json::Value)],
    ) -> ObserverAction {
        let mut worst = ObserverAction::Continue;

        for (_, name, input) in tool_uses {
            let sig = tool_signature(name, input);
            let count = self.signatures.entry(sig).or_insert(0);
            *count += 1;

            if *count >= ABORT_THRESHOLD {
                return ObserverAction::AbortTurn(format!(
                    "Loop detected: tool '{name}' called {count} cumulative times \
                     with similar arguments. Aborting to prevent waste.",
                ));
            }
            if *count >= WARN_THRESHOLD {
                worst = ObserverAction::InjectWarning(format!(
                    "[WARNING: Tool '{name}' has been called {count} times with similar \
                     arguments. You may be stuck in a loop. Try a different \
                     approach or ask the user for help.]",
                ));
            }
        }

        worst
    }

    fn on_user_input(&mut self) {
        self.signatures.clear();
    }
}

/// Build a stable signature from tool name + truncated input JSON.
fn tool_signature(name: &str, input: &serde_json::Value) -> String {
    let json = input.to_string();
    let truncated = if json.len() > SIGNATURE_INPUT_LIMIT {
        &json[..SIGNATURE_INPUT_LIMIT]
    } else {
        &json
    };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    truncated.hash(&mut hasher);
    format!("{name}|{:x}", hasher.finish())
}
