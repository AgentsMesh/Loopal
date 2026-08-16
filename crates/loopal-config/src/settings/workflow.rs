mod bounds;

use loopal_provider_api::{EffortLevel, ThinkingConfig};
use serde::{Deserialize, Serialize};

use super::core::Settings;
pub use bounds::{WorkflowLimits, WorkflowTiming};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrchestrationPolicy {
    #[default]
    Off,
    Explicit,
    Proactive,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowPlannerProfile {
    #[default]
    Default,
    Ultracode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowPreset {
    Ultracode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkflowSettings {
    pub policy: OrchestrationPolicy,
    pub execution_enabled: bool,
    pub planner_profile: WorkflowPlannerProfile,
    pub limits: WorkflowLimits,
    pub timing: WorkflowTiming,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preset: Option<WorkflowPreset>,
}

impl Default for WorkflowSettings {
    fn default() -> Self {
        Self {
            policy: OrchestrationPolicy::Off,
            execution_enabled: false,
            planner_profile: WorkflowPlannerProfile::Default,
            limits: WorkflowLimits::default(),
            timing: WorkflowTiming::default(),
            preset: None,
        }
    }
}

impl WorkflowSettings {
    pub fn sanitize(mut self) -> (Self, Vec<String>) {
        let mut warnings = Vec::new();
        self.limits.sanitize_into(&mut warnings);
        self.timing.sanitize_into(&mut warnings);
        if self.limits.max_parallel > self.limits.max_nodes {
            warnings.push("workflow.limits.max_parallel exceeds max_nodes; clamped".into());
            self.limits.max_parallel = self.limits.max_nodes;
        }
        if self.timing.attempt_timeout_secs > self.timing.run_deadline_secs {
            warnings.push(
                "workflow.timing.attempt_timeout_secs exceeds run_deadline_secs; clamped".into(),
            );
            self.timing.attempt_timeout_secs = self.timing.run_deadline_secs;
        }
        (self, warnings)
    }

    pub fn validate(&self) -> Result<(), String> {
        for (field, value) in self.nonzero_values() {
            if value == 0 {
                return Err(format!("{field} must be greater than zero"));
            }
        }
        if self.limits.max_attempts < self.limits.max_nodes {
            return Err("limits.max_attempts must be at least limits.max_nodes".into());
        }
        Ok(())
    }

    fn nonzero_values(&self) -> [(&'static str, u64); 8] {
        [
            ("limits.max_nodes", u64::from(self.limits.max_nodes)),
            ("limits.max_parallel", u64::from(self.limits.max_parallel)),
            ("limits.max_attempts", u64::from(self.limits.max_attempts)),
            ("limits.max_output_bytes", self.limits.max_output_bytes),
            ("timing.run_deadline_secs", self.timing.run_deadline_secs),
            (
                "timing.attempt_timeout_secs",
                self.timing.attempt_timeout_secs,
            ),
            ("timing.cancel_grace_secs", self.timing.cancel_grace_secs),
            (
                "timing.recovery_grace_secs",
                self.timing.recovery_grace_secs,
            ),
        ]
    }

    fn apply_preset(&mut self) -> Option<ThinkingConfig> {
        let WorkflowPreset::Ultracode = self.preset?;
        self.policy = OrchestrationPolicy::Proactive;
        // Execution remains an independent, release-gated opt-in. Selecting a
        // preset must not make the production coordinator executable by itself.
        self.planner_profile = WorkflowPlannerProfile::Ultracode;
        self.limits = WorkflowLimits::ULTRACODE;
        self.timing = WorkflowTiming::ULTRACODE;
        Some(ThinkingConfig::Effort {
            level: EffortLevel::Max,
        })
    }
}

pub struct WorkflowPresetResolution {
    pub settings: Settings,
    pub recommended_thinking: Option<ThinkingConfig>,
}

impl Settings {
    pub fn resolve_workflow_preset(&self) -> WorkflowPresetResolution {
        let mut settings = self.clone();
        let recommended_thinking = settings.workflow.apply_preset();
        WorkflowPresetResolution {
            settings,
            recommended_thinking,
        }
    }
}
