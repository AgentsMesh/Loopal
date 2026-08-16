use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use tokio::sync::Notify;

use super::support::harness;
use crate::types::AgentExecutionRef;
use crate::workflow::scheduler::{
    WorkflowActivationFailure, WorkflowCleanupStatus, WorkflowSpawner,
};

include!("lifecycle_ownership/activation.rs");
include!("lifecycle_ownership/cleanup.rs");
