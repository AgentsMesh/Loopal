use std::sync::{Arc, atomic::Ordering};
use std::time::Duration;

use super::super::AttemptPhase;
use super::requests::request;
use super::support::{harness, wait_for};
use crate::types::AgentExecutionRef;
use crate::workflow::scheduler::{
    WorkflowActivationFailure, WorkflowCleanupStatus, WorkflowSpawner, WorkflowStopStatus,
};

include!("control/interrupt.rs");
include!("control/lifecycle.rs");
