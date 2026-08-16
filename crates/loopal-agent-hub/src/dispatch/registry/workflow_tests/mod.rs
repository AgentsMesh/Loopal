use std::sync::Arc;

use loopal_ipc::Connection;
use loopal_ipc::protocol::methods;
use loopal_protocol::{
    QualifiedAddress, ROOT_AGENT_NAME, WORKFLOW_SPEC_V1, WorkflowAgentNode,
    WorkflowAttemptCapability, WorkflowAttemptId, WorkflowCancelRequest, WorkflowGetRequest,
    WorkflowLimits, WorkflowNodeId, WorkflowOutputContract, WorkflowPermissionCausation,
    WorkflowRequestId, WorkflowRunId, WorkflowSpec, WorkflowStartLookupRequest,
    WorkflowStartRequest, WorkflowWaitRequest, WorkflowWorkerHandshakeRequest,
    WorkflowWorkerProfileRef,
};
use tokio::sync::mpsc;

use super::*;
use crate::request_principal::{AgentPrincipal, HubRequestPrincipal};
use crate::types::{AgentExecutionRef, AgentOrigin, AgentRuntimeFacts, SpawnAuthority};

include!("owner.rs");
include!("root_dispatch.rs");
include!("worker_support.rs");
include!("worker_authority.rs");
include!("stale_authority.rs");
