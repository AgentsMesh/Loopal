#![allow(dead_code)]

//! Small branch-coverage crate for the production workflow-runtime latch.

extern crate self as anyhow;
extern crate self as loopal_agent_hub;
extern crate self as loopal_config;
extern crate self as loopal_storage;

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[derive(Debug)]
pub struct Error(String);

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

#[macro_export]
macro_rules! anyhow {
    ($($argument:tt)*) => {
        $crate::Error(format!($($argument)*))
    };
}

// The root unit suite exercises the real config crate; this producer only
// mirrors the fields and preset transition consumed by the latch tests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OrchestrationPolicy {
    #[default]
    Off,
    Explicit,
    Proactive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowPreset {
    Ultracode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowSettings {
    pub policy: OrchestrationPolicy,
    pub execution_enabled: bool,
    pub preset: Option<WorkflowPreset>,
}

impl Default for WorkflowSettings {
    fn default() -> Self {
        Self {
            policy: OrchestrationPolicy::Off,
            execution_enabled: false,
            preset: None,
        }
    }
}

#[derive(Clone, Default)]
pub struct Settings {
    pub workflow: WorkflowSettings,
}

pub struct WorkflowPresetResolution {
    pub settings: Settings,
}

impl Settings {
    pub fn resolve_workflow_preset(&self) -> WorkflowPresetResolution {
        let mut settings = self.clone();
        if settings.workflow.preset == Some(WorkflowPreset::Ultracode) {
            settings.workflow.policy = OrchestrationPolicy::Proactive;
        }
        WorkflowPresetResolution { settings }
    }
}

// The production module only needs these construction boundaries to compile.
// Runtime integration remains covered by the root bootstrap test producers.
pub struct SessionStore;

impl SessionStore {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self)
    }
}

pub mod workflow {
    use super::{Arc, CONSTRUCTION_FAILS, SessionStore, WorkflowSettings};
    use crate::states::Hub;

    pub struct WorkflowRuntime;

    impl WorkflowRuntime {
        pub async fn new_production(
            _hub: Hub,
            _sessions: Arc<SessionStore>,
            _settings: &WorkflowSettings,
        ) -> anyhow::Result<Option<Self>> {
            if CONSTRUCTION_FAILS.load(std::sync::atomic::Ordering::SeqCst) {
                Err(crate::Error("synthetic construction failure".into()))
            } else {
                Ok(None)
            }
        }
    }
}

static CONSTRUCTION_FAILS: AtomicBool = AtomicBool::new(false);
static AGENT_SHUTDOWNS: AtomicUsize = AtomicUsize::new(0);

pub mod states {
    use crate::workflow::WorkflowRuntime;

    #[derive(Clone)]
    pub struct Hub;

    pub struct AgentProcess;

    impl AgentProcess {
        pub async fn shutdown(self) -> anyhow::Result<()> {
            crate::AGENT_SHUTDOWNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
    }

    pub struct RootPending {
        pub(crate) hub: Hub,
        pub(crate) hub_token: (),
        pub(crate) agent_proc: AgentProcess,
        pub(crate) client_conn: (),
        pub(crate) workflow_runtime: Option<WorkflowRuntime>,
    }
}

#[path = "../../src/bootstrap/hub/typestate/workflow_runtime.rs"]
mod workflow_runtime;

#[cfg(test)]
mod coverage_tests {
    use super::*;
    use crate::states::{AgentProcess, Hub, RootPending};
    use crate::workflow::WorkflowRuntime;

    fn pending(workflow_runtime: Option<WorkflowRuntime>) -> RootPending {
        RootPending {
            hub: Hub,
            hub_token: (),
            agent_proc: AgentProcess,
            client_conn: (),
            workflow_runtime,
        }
    }

    fn enabled() -> WorkflowSettings {
        WorkflowSettings {
            policy: OrchestrationPolicy::Explicit,
            execution_enabled: true,
            preset: None,
        }
    }

    #[tokio::test]
    async fn installation_reuses_an_existing_runtime_and_rolls_back_construction_failure() {
        let existing = pending(Some(WorkflowRuntime))
            .install_workflow_runtime(&enabled())
            .await
            .unwrap();
        assert!(existing.workflow_runtime.is_some());

        AGENT_SHUTDOWNS.store(0, Ordering::SeqCst);
        CONSTRUCTION_FAILS.store(true, Ordering::SeqCst);
        let error = match pending(None).install_workflow_runtime(&enabled()).await {
            Ok(_) => panic!("construction failure must abort installation"),
            Err(error) => error,
        };
        CONSTRUCTION_FAILS.store(false, Ordering::SeqCst);

        assert!(
            error
                .to_string()
                .contains("workflow runtime initialization failed")
        );
        assert_eq!(AGENT_SHUTDOWNS.load(Ordering::SeqCst), 1);
    }
}
