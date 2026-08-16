mod aggregate;
mod bg;
mod compact;
mod effect;
mod interactive;
mod lifecycle;
mod observable;
mod question;
mod stream;
mod tool;
mod workflow;
use crate::state::SessionViewState;
pub(crate) use effect::MutationEffect;
use loopal_protocol::AgentEventPayload;

pub(crate) fn mutate(state: &mut SessionViewState, event: &AgentEventPayload) -> MutationEffect {
    use AgentEventPayload::*;
    match event {
        Started => observable::started(state),
        Running => observable::running(state),
        AwaitingInput => observable::awaiting_input(state),
        Finished => observable::finished(state),
        Interrupted => observable::interrupted(state),
        Error { message } => observable::error(state, message),
        ProviderWarning { message } => observable::provider_warning(state, message),
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
        ToolPermissionRequest {
            id,
            name,
            input,
            permission_intent,
        } => interactive::tool_permission_request(
            state,
            id,
            name,
            input,
            permission_intent.as_deref(),
        ),
        ToolPermissionResolved { id } => interactive::tool_permission_resolved(state, id),
        PlanApprovalRequest {
            id,
            plan_content,
            plan_path,
        } => interactive::plan_approval_request(state, id, plan_content, plan_path),
        PlanApprovalResolved { id } => interactive::plan_approval_resolved(state, id),
        UserQuestionRequest {
            id,
            logical_id,
            questions,
            classifier_running,
        } => question::user_question_request(state, id, logical_id, questions, *classifier_running),
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
            skill_info,
        } => interactive::user_message_queued(
            state,
            envelope_id,
            content,
            *image_count,
            skill_info.clone(),
        ),
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
            reason,
        } => interactive::auto_continuation(state, *continuation, *max_continuations, reason),
        Compacted(s) => interactive::compacted(
            state,
            s.kept,
            s.summarized,
            s.tokens_before,
            s.tokens_after,
            &s.strategy,
            s.files_rehydrated,
        ),
        Rewound { remaining_turns } => stream::rewound(state, *remaining_turns),
        ServerToolUse { id, name, input } => tool::server_tool_use(state, id, name, input),
        ServerToolResult {
            tool_use_id,
            content,
        } => tool::server_tool_result(state, tool_use_id, content),
        ServerToolDiscarded {
            tool_use_id,
            reason,
        } => tool::server_tool_discarded(state, tool_use_id, *reason),
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
        PermissionModeChanged { mode } => lifecycle::permission_mode_changed(state, mode),
        DecisionModeChanged { mode } => lifecycle::decision_mode_changed(state, mode),
        SandboxPolicyChanged { policy } => lifecycle::sandbox_policy_changed(state, policy),
        ContinuationGateChanged(summary) => lifecycle::continuation_gate_changed(state, summary),
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
        SessionHistoryLoaded(history) => aggregate::session_history_loaded(state, history),
        ThreadGoalUpdated { goal, .. } => aggregate::thread_goal_updated(state, goal),
        WorkflowRunChanged(summary) => workflow::workflow_changed(state, summary),
        HubDegraded { since_unix_ms } => aggregate::hub_degraded(state, *since_unix_ms),
        HubRecovered { .. } => aggregate::hub_recovered(state),
        MessageRouted { .. }
        | InboxConsumed { .. }
        | TurnDiffSummary { .. }
        | SessionResumeWarnings { .. }
        | QuestionDecided { .. }
        | DegenerationDetected(_)
        | ContinuationSkipped { .. }
        | TurnCancelled { .. } => MutationEffect::NoOp,
        CompactProgress { phase, detail } => compact::progress(state, *phase, detail.as_deref()),
    }
}
