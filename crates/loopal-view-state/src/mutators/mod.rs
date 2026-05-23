mod aggregate;
mod bg;
mod compact;
mod interactive;
mod lifecycle;
mod observable;
mod question;
mod stream;
mod tool;

use loopal_protocol::AgentEventPayload;

use crate::state::SessionViewState;

/// Outcome of a single mutator. Each mutator function returns one of these
/// directly — there is no central "which event ends a turn" table. The
/// mutator's domain logic decides the effect, and the reducer consumes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MutationEffect {
    /// Event was a no-op (unrecognised, duplicate, empty-id rejected).
    NoOp,
    /// State changed; no turn lifecycle implication.
    Mutated,
    /// State changed AND this mutator marks turn-end — reducer must
    /// reconcile still-active tool invocations into terminal states.
    MutatedEndedTurn,
}

impl MutationEffect {
    pub fn changed(self) -> bool {
        !matches!(self, Self::NoOp)
    }

    pub fn requires_turn_end_reconcile(self) -> bool {
        matches!(self, Self::MutatedEndedTurn)
    }
}

pub(crate) fn mutate(state: &mut SessionViewState, event: &AgentEventPayload) -> MutationEffect {
    use AgentEventPayload::*;
    match event {
        Started => observable::started(state),
        Running => observable::running(state),
        AwaitingInput => observable::awaiting_input(state),
        Finished => observable::finished(state),
        Interrupted => observable::interrupted(state),
        Error { message } => observable::error(state, message),
        ToolCall { id, name, input } => tool::tool_call(state, id, name, input),
        ToolResult {
            id,
            name,
            result,
            is_error,
            metadata,
            ..
        } => tool::tool_result(state, id, name, result, *is_error, metadata.clone()),
        ToolBatchStart { tool_ids } => tool::tool_batch_start(state, tool_ids),
        ToolProgress {
            id, output_tail, ..
        } => tool::tool_progress(state, id, output_tail),
        ToolPermissionRequest { id, name, input } => {
            interactive::tool_permission_request(state, id, name, input)
        }
        ToolPermissionResolved { id } => interactive::tool_permission_resolved(state, id),
        UserQuestionRequest {
            id,
            questions,
            classifier_running,
        } => question::user_question_request(state, id, questions, *classifier_running),
        UserQuestionResolved { id, .. } => question::user_question_resolved(state, id),
        ClassifierProgress { id, elapsed_ms } => {
            question::classifier_progress(state, id, *elapsed_ms)
        }
        ClassifierFailed { id, reason } => question::classifier_failed(state, id, reason),
        ClassifierCompleted { id, answers, .. } => {
            question::classifier_completed(state, id, answers)
        }
        UserMessageQueued {
            envelope_id,
            content,
            image_count,
        } => interactive::user_message_queued(state, envelope_id, content, *image_count),
        Stream { text } => stream::stream(state, text),
        ThinkingStream { text } => stream::thinking_stream(state, text),
        ThinkingComplete { token_count } => stream::thinking_complete(state, *token_count),
        TokenUsage {
            input_tokens,
            output_tokens,
            context_window,
            cache_creation_input_tokens,
            cache_read_input_tokens,
            ..
        } => observable::token_usage(
            state,
            *input_tokens,
            *output_tokens,
            *context_window,
            *cache_creation_input_tokens,
            *cache_read_input_tokens,
        ),
        RetryError {
            message,
            attempt,
            max_attempts,
        } => stream::retry_error(state, message, *attempt, *max_attempts),
        RetryCleared => stream::retry_cleared(state),
        AutoContinuation {
            continuation,
            max_continuations,
        } => interactive::auto_continuation(state, *continuation, *max_continuations),
        Compacted(s) => interactive::compacted(
            state,
            s.kept,
            s.removed,
            s.tokens_before,
            s.tokens_after,
            &s.strategy,
        ),
        Rewound { remaining_turns } => stream::rewound(state, *remaining_turns),
        ServerToolUse { id, name, input } => tool::server_tool_use(state, id, name, input),
        ServerToolResult {
            tool_use_id,
            content,
        } => tool::server_tool_result(state, tool_use_id, content),
        InboxEnqueued {
            envelope_id,
            source,
            content,
            summary,
        } => interactive::inbox_enqueued(state, envelope_id, source, content, summary.as_deref()),
        PermissionDecided {
            tool_name,
            decision,
            reason,
            duration_ms,
        } => interactive::permission_decided(state, tool_name, decision, reason, *duration_ms),
        ModeChanged { mode } => lifecycle::mode_changed(state, mode),
        ModelChanged { model } => lifecycle::model_changed(state, model),
        ThinkingChanged { thinking_config } => lifecycle::thinking_changed(state, thinking_config),
        Cleared { context_window } => lifecycle::cleared(state, *context_window),
        TurnCompleted(_) => observable::turn_completed(state),
        TasksChanged { tasks } => aggregate::tasks_changed(state, tasks),
        CronsChanged { crons } => aggregate::crons_changed(state, crons),
        BgTaskSpawned {
            id,
            description,
            created_at_unix_ms,
        } => bg::spawned(state, id, description, *created_at_unix_ms),
        BgTaskOutput { id, output_delta } => bg::output(state, id, output_delta),
        BgTaskCompleted {
            id,
            status,
            exit_code,
            output,
        } => bg::completed(state, id, *status, *exit_code, output),
        McpStatusReport { servers } => aggregate::mcp_status(state, servers),
        SubAgentSpawned(s) => aggregate::sub_agent_spawned(state, &s.name),
        SessionResumed { session_id, .. } => aggregate::session_resumed(state, session_id),
        ThreadGoalUpdated { goal, .. } => aggregate::thread_goal_updated(state, goal),
        HubDegraded { since_unix_ms } => aggregate::hub_degraded(state, *since_unix_ms),
        HubRecovered { .. } => aggregate::hub_recovered(state),
        MessageRouted { .. }
        | InboxConsumed { .. }
        | TurnDiffSummary { .. }
        | SessionResumeWarnings { .. }
        | QuestionDecided { .. }
        | DegenerationDetected(_)
        | ContinuationGateChanged(_) => MutationEffect::NoOp,
        CompactProgress { phase, detail } => compact::progress(state, *phase, detail.as_deref()),
    }
}
