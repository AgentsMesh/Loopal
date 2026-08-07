use loopal_protocol::CompactPhase;

use crate::state::SessionViewState;

use super::MutationEffect;

pub(super) fn progress(
    state: &mut SessionViewState,
    phase: CompactPhase,
    detail: Option<&str>,
) -> MutationEffect {
    let conv = &mut state.agent.conversation;
    conv.mark_active();
    match phase {
        CompactPhase::Done => {
            conv.compact_banner = None;
            // Retry is a nested compaction sub-state. Closing the parent
            // operation also repairs a lost RetryCleared event.
            conv.retry_banner = None;
        }
        phase => {
            conv.compact_banner = Some(format_banner(phase, detail));
        }
    }
    MutationEffect::Mutated
}

fn format_banner(phase: CompactPhase, detail: Option<&str>) -> String {
    let label = match phase {
        CompactPhase::Microcompact => "⠏ microcompacting idle tool results",
        CompactPhase::Summarize => "⠙ summarizing context",
        CompactPhase::Rehydrate => "⠹ rehydrating files",
        CompactPhase::Done => "✓ compact done",
    };
    match detail {
        Some(d) if !d.is_empty() => format!("{label} — {d}"),
        _ => label.to_string(),
    }
}
