use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkflowLimits {
    pub max_nodes: u32,
    pub max_parallel: u32,
    pub max_attempts: u32,
    /// Maximum bytes in bounded redacted text or schema-validated JSON output.
    pub max_output_bytes: u64,
}

impl WorkflowLimits {
    pub const ABSOLUTE_MAX_NODES: u32 = 512;
    pub const ABSOLUTE_MAX_PARALLEL: u32 = 64;
    pub const ABSOLUTE_MAX_ATTEMPTS: u32 = 2048;
    pub const ABSOLUTE_MAX_OUTPUT_BYTES: u64 = loopal_protocol::MAX_WORKFLOW_OUTPUT_BYTES as u64;

    pub(super) const ULTRACODE: Self = Self {
        max_nodes: 128,
        max_parallel: 16,
        max_attempts: 384,
        max_output_bytes: 1024 * 1024,
    };

    pub(super) fn sanitize_into(&mut self, warnings: &mut Vec<String>) {
        clamp_u32(
            &mut self.max_nodes,
            Self::ABSOLUTE_MAX_NODES,
            "workflow.limits.max_nodes",
            warnings,
        );
        clamp_u32(
            &mut self.max_parallel,
            Self::ABSOLUTE_MAX_PARALLEL,
            "workflow.limits.max_parallel",
            warnings,
        );
        clamp_u32(
            &mut self.max_attempts,
            Self::ABSOLUTE_MAX_ATTEMPTS,
            "workflow.limits.max_attempts",
            warnings,
        );
        clamp_u64(
            &mut self.max_output_bytes,
            Self::ABSOLUTE_MAX_OUTPUT_BYTES,
            "workflow.limits.max_output_bytes",
            warnings,
        );
    }
}

impl Default for WorkflowLimits {
    fn default() -> Self {
        Self {
            max_nodes: 32,
            max_parallel: 4,
            max_attempts: 64,
            max_output_bytes: 256 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkflowTiming {
    pub run_deadline_secs: u64,
    pub attempt_timeout_secs: u64,
    pub cancel_grace_secs: u64,
    pub recovery_grace_secs: u64,
}

impl WorkflowTiming {
    pub const ABSOLUTE_MAX_RUN_DEADLINE_SECS: u64 = 24 * 60 * 60;
    pub const ABSOLUTE_MAX_ATTEMPT_TIMEOUT_SECS: u64 = 4 * 60 * 60;
    pub const ABSOLUTE_MAX_CANCEL_GRACE_SECS: u64 = 5 * 60;
    pub const ABSOLUTE_MAX_RECOVERY_GRACE_SECS: u64 = 30 * 60;

    pub(super) const ULTRACODE: Self = Self {
        run_deadline_secs: 4 * 60 * 60,
        attempt_timeout_secs: 60 * 60,
        cancel_grace_secs: 30,
        recovery_grace_secs: 5 * 60,
    };

    pub(super) fn sanitize_into(&mut self, warnings: &mut Vec<String>) {
        clamp_u64(
            &mut self.run_deadline_secs,
            Self::ABSOLUTE_MAX_RUN_DEADLINE_SECS,
            "workflow.timing.run_deadline_secs",
            warnings,
        );
        clamp_u64(
            &mut self.attempt_timeout_secs,
            Self::ABSOLUTE_MAX_ATTEMPT_TIMEOUT_SECS,
            "workflow.timing.attempt_timeout_secs",
            warnings,
        );
        clamp_u64(
            &mut self.cancel_grace_secs,
            Self::ABSOLUTE_MAX_CANCEL_GRACE_SECS,
            "workflow.timing.cancel_grace_secs",
            warnings,
        );
        clamp_u64(
            &mut self.recovery_grace_secs,
            Self::ABSOLUTE_MAX_RECOVERY_GRACE_SECS,
            "workflow.timing.recovery_grace_secs",
            warnings,
        );
    }
}

impl Default for WorkflowTiming {
    fn default() -> Self {
        Self {
            run_deadline_secs: 60 * 60,
            attempt_timeout_secs: 15 * 60,
            cancel_grace_secs: 15,
            recovery_grace_secs: 2 * 60,
        }
    }
}

fn clamp_u32(value: &mut u32, max: u32, field: &str, warnings: &mut Vec<String>) {
    if *value > max {
        warnings.push(format!("{field}={} exceeds max {max}; clamped", *value));
        *value = max;
    }
}

fn clamp_u64(value: &mut u64, max: u64, field: &str, warnings: &mut Vec<String>) {
    if *value > max {
        warnings.push(format!("{field}={} exceeds max {max}; clamped", *value));
        *value = max;
    }
}
