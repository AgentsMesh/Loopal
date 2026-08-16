use loopal_output_guard::FinalSinkRedactionSeed;

#[derive(Clone)]
pub(in crate::workflow) struct WorkflowRuntimeConfig {
    pub(crate) ceilings: WorkflowTrustedCeilings,
    pub(crate) cancel_grace_ms: u64,
    pub(crate) recovery_grace_ms: u64,
    pub(crate) redaction_seed: FinalSinkRedactionSeed,
}

impl WorkflowRuntimeConfig {
    pub(crate) fn test_default() -> Self {
        Self {
            ceilings: WorkflowTrustedCeilings::PROTOCOL_MAXIMUM,
            cancel_grace_ms: 1_000,
            recovery_grace_ms: 0,
            redaction_seed: FinalSinkRedactionSeed::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::workflow) struct WorkflowTrustedCeilings {
    pub(crate) max_nodes: u32,
    pub(crate) max_parallel: u32,
    pub(crate) max_attempts: u32,
    pub(crate) max_output_bytes: u32,
    pub(crate) run_deadline_ms: u64,
    pub(crate) attempt_timeout_ms: u64,
}

impl WorkflowTrustedCeilings {
    pub(crate) const PROTOCOL_MAXIMUM: Self = Self {
        max_nodes: loopal_protocol::MAX_WORKFLOW_NODES,
        max_parallel: loopal_protocol::MAX_WORKFLOW_PARALLELISM,
        max_attempts: loopal_protocol::MAX_WORKFLOW_ATTEMPTS,
        max_output_bytes: loopal_protocol::MAX_WORKFLOW_OUTPUT_BYTES,
        run_deadline_ms: loopal_protocol::MAX_WORKFLOW_RUN_DEADLINE_MS,
        attempt_timeout_ms: loopal_protocol::MAX_WORKFLOW_ATTEMPT_TIMEOUT_MS,
    };

    pub(crate) fn from_settings(settings: &loopal_config::WorkflowSettings) -> Self {
        Self {
            max_nodes: settings.limits.max_nodes,
            max_parallel: settings.limits.max_parallel,
            max_attempts: settings.limits.max_attempts,
            max_output_bytes: u32::try_from(settings.limits.max_output_bytes)
                .expect("sanitized workflow output limit fits protocol bound"),
            run_deadline_ms: settings.timing.run_deadline_secs.saturating_mul(1_000),
            attempt_timeout_ms: settings.timing.attempt_timeout_secs.saturating_mul(1_000),
        }
    }

    pub(crate) fn validate(
        self,
        limits: &loopal_protocol::WorkflowLimits,
    ) -> Result<(), super::super::WorkflowCoordinatorError> {
        let checks = [
            (limits.max_nodes <= self.max_nodes, "max_nodes"),
            (limits.max_parallel <= self.max_parallel, "max_parallel"),
            (limits.max_attempts <= self.max_attempts, "max_attempts"),
            (
                limits.max_output_bytes <= self.max_output_bytes,
                "max_output_bytes",
            ),
            (
                limits.run_deadline_ms <= self.run_deadline_ms,
                "run_deadline_ms",
            ),
            (
                limits.attempt_timeout_ms <= self.attempt_timeout_ms,
                "attempt_timeout_ms",
            ),
        ];
        checks
            .into_iter()
            .find_map(|(valid, field)| (!valid).then_some(field))
            .map_or(Ok(()), |field| {
                Err(super::super::WorkflowCoordinatorError::TrustedLimitExceeded(field))
            })
    }
}
