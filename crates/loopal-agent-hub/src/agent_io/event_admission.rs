use loopal_protocol::AgentEventPayload;

pub(super) fn agent_may_emit(payload: &AgentEventPayload) -> bool {
    match payload {
        AgentEventPayload::ToolPermissionRequest { .. }
        | AgentEventPayload::PlanApprovalRequest { .. }
        | AgentEventPayload::PlanApprovalResolved { .. }
        | AgentEventPayload::MessageRouted { .. }
        | AgentEventPayload::InboxEnqueued { .. }
        | AgentEventPayload::UserMessageQueued { .. }
        | AgentEventPayload::UserQuestionRequest { .. }
        | AgentEventPayload::ToolPermissionResolved { .. }
        | AgentEventPayload::UserQuestionResolved { .. }
        | AgentEventPayload::SubAgentSpawned(_)
        | AgentEventPayload::WorkflowRunChanged(_) => false,
        AgentEventPayload::Stream { .. }
        | AgentEventPayload::ThinkingStream { .. }
        | AgentEventPayload::ThinkingComplete { .. }
        | AgentEventPayload::ToolCall { .. }
        | AgentEventPayload::ToolResult { .. }
        | AgentEventPayload::ToolProgress { .. }
        | AgentEventPayload::ToolBatchStart { .. }
        | AgentEventPayload::Error { .. }
        | AgentEventPayload::ProviderWarning { .. }
        | AgentEventPayload::RetryError { .. }
        | AgentEventPayload::RetryCleared
        | AgentEventPayload::AwaitingInput
        | AgentEventPayload::AutoContinuation { .. }
        | AgentEventPayload::TokenUsage { .. }
        | AgentEventPayload::ModeChanged { .. }
        | AgentEventPayload::ModelChanged { .. }
        | AgentEventPayload::ThinkingChanged { .. }
        | AgentEventPayload::PermissionModeChanged { .. }
        | AgentEventPayload::DecisionModeChanged { .. }
        | AgentEventPayload::SandboxPolicyChanged { .. }
        | AgentEventPayload::Cleared { .. }
        | AgentEventPayload::Started
        | AgentEventPayload::Running
        | AgentEventPayload::Finished
        | AgentEventPayload::InboxConsumed { .. }
        | AgentEventPayload::ClassifierProgress { .. }
        | AgentEventPayload::ClassifierFailed { .. }
        | AgentEventPayload::ClassifierCompleted { .. }
        | AgentEventPayload::Rewound { .. }
        | AgentEventPayload::Compacted(_)
        | AgentEventPayload::CompactProgress { .. }
        | AgentEventPayload::Interrupted
        | AgentEventPayload::TurnDiffSummary { .. }
        | AgentEventPayload::ServerToolUse { .. }
        | AgentEventPayload::ServerToolResult { .. }
        | AgentEventPayload::ServerToolDiscarded { .. }
        | AgentEventPayload::PermissionDecided { .. }
        | AgentEventPayload::QuestionDecided { .. }
        | AgentEventPayload::SessionResumed { .. }
        | AgentEventPayload::SessionHistoryLoaded(_)
        | AgentEventPayload::SessionResumeWarnings { .. }
        | AgentEventPayload::BgTaskSpawned { .. }
        | AgentEventPayload::BgTaskOutput { .. }
        | AgentEventPayload::BgTaskCompleted { .. }
        | AgentEventPayload::TurnCompleted(_)
        | AgentEventPayload::McpStatusReport { .. }
        | AgentEventPayload::TasksChanged { .. }
        | AgentEventPayload::CronsChanged { .. }
        | AgentEventPayload::ThreadGoalUpdated { .. }
        | AgentEventPayload::HubDegraded { .. }
        | AgentEventPayload::HubRecovered { .. }
        | AgentEventPayload::DegenerationDetected(_)
        | AgentEventPayload::ContinuationGateChanged(_)
        | AgentEventPayload::ContinuationSkipped { .. }
        | AgentEventPayload::TurnCancelled { .. } => true,
    }
}
