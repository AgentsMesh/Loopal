mod aggregate;
mod bg;
mod interactive;
mod observable;
mod question;
mod stream;
mod tool;

use loopal_protocol::AgentEventPayload;

use crate::state::SessionViewState;

pub(crate) fn mutate(state: &mut SessionViewState, event: &AgentEventPayload) -> bool {
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
            duration_ms,
            metadata,
        } => tool::tool_result(
            state,
            id,
            name,
            result,
            *is_error,
            *duration_ms,
            metadata.clone(),
        ),
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
            message_id,
            content,
            image_count,
        } => interactive::user_message_queued(state, message_id, content, *image_count),
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
        Compacted {
            kept,
            removed,
            tokens_before,
            tokens_after,
            strategy,
        } => interactive::compacted(
            state,
            *kept,
            *removed,
            *tokens_before,
            *tokens_after,
            strategy,
        ),
        Rewound { remaining_turns } => stream::rewound(state, *remaining_turns),
        ServerToolUse { id, name, input } => tool::server_tool_use(state, id, name, input),
        ServerToolResult {
            tool_use_id,
            content,
        } => tool::server_tool_result(state, tool_use_id, content),
        InboxEnqueued {
            message_id,
            source,
            content,
            summary,
        } => interactive::inbox_enqueued(state, message_id, source, content, summary.as_deref()),
        PermissionDecided {
            tool_name,
            decision,
            reason,
            duration_ms,
        } => interactive::permission_decided(state, tool_name, decision, reason, *duration_ms),
        QuestionDecided {
            question_count,
            duration_ms,
            reason,
            source,
        } => interactive::question_decided(state, *question_count, reason, *duration_ms, *source),
        ModeChanged { mode } => observable::mode_changed(state, mode),
        TurnCompleted { .. } => observable::turn_completed(state),
        TasksChanged { tasks } => aggregate::tasks_changed(state, tasks),
        CronsChanged { crons } => aggregate::crons_changed(state, crons),
        BgTaskSpawned { id, description } => bg::spawned(state, id, description),
        BgTaskOutput { id, output_delta } => bg::output(state, id, output_delta),
        BgTaskCompleted {
            id,
            status,
            exit_code,
            output,
        } => bg::completed(state, id, *status, *exit_code, output),
        McpStatusReport { servers } => aggregate::mcp_status(state, servers),
        SubAgentSpawned { name, .. } => aggregate::sub_agent_spawned(state, name),
        SessionResumed { session_id, .. } => aggregate::session_resumed(state, session_id),
        ThreadGoalUpdated { goal, .. } => aggregate::thread_goal_updated(state, goal),
        MessageRouted { .. }
        | InboxConsumed { .. }
        | TurnDiffSummary { .. }
        | SessionResumeWarnings { .. } => false,
    }
}
