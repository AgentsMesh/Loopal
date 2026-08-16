use std::sync::Arc;
use std::time::Duration;

use loopal_agent::workflow_control::WorkflowStartControlError;
use loopal_config::{OrchestrationPolicy, WorkflowSettings};
use loopal_runtime::workflow_input::{WorkflowInputDisposition, WorkflowInputHandler};

use super::test_support::*;

include!("workflow_input_tests/policy_and_dedup.rs");
include!("workflow_input_tests/indeterminate.rs");
