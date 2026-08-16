fn owner() -> WorkflowOwner {
    WorkflowOwner::new(
        "scheduler-coverage-session",
        QualifiedAddress::local("root"),
    )
}
fn spec() -> WorkflowSpec {
    let node_id = WorkflowNodeId::new("node");
    WorkflowSpec {
        version: WORKFLOW_SPEC_V1,
        run_goal: "exercise scheduler invariants".into(),
        nodes: vec![WorkflowAgentNode {
            id: node_id.clone(),
            dependencies: Vec::new(),
            task: "run the scheduler case".into(),
            worker_profile: WorkflowWorkerProfileRef::new("default"),
        }],
        limits: WorkflowLimits {
            max_nodes: 1,
            max_parallel: 1,
            max_attempts: 1,
            run_deadline_ms: 1_000,
            attempt_timeout_ms: 100,
            max_output_bytes: 1_024,
        },
        output_node: node_id,
        output_contract: WorkflowOutputContract::Text { max_bytes: 512 },
    }
}

fn apply(run: WorkflowRunSnapshot, payload: WorkflowEventPayload) -> WorkflowRunSnapshot {
    let occurred_at = run.updated_at_unix_ms.saturating_add(1);
    apply_payload(&run, payload, occurred_at).unwrap().1
}

fn planned_run(run_id: &str) -> WorkflowRunSnapshot {
    WorkflowRunSnapshot::planned(
        WorkflowRunId::new(run_id),
        QualifiedAddress::local("root"),
        spec(),
        10,
    )
}

fn validated_run(run_id: &str) -> WorkflowRunSnapshot {
    apply(planned_run(run_id), WorkflowEventPayload::SpecValidated)
}

fn running_ready_run(run_id: &str) -> WorkflowRunSnapshot {
    apply(validated_run(run_id), WorkflowEventPayload::RunStarted)
}

fn dispatching_run(run_id: &str, attempt_id: &str) -> (WorkflowRunSnapshot, AttemptKey) {
    let run = running_ready_run(run_id);
    let key = AttemptKey {
        run_id: run.id.clone(),
        node_id: WorkflowNodeId::new("node"),
        attempt_id: WorkflowAttemptId::new(attempt_id),
    };
    let capability = WorkflowAttemptCapability::parse("44".repeat(32)).unwrap();
    let run = apply(
        run,
        WorkflowEventPayload::DispatchIntended {
            node_id: key.node_id.clone(),
            attempt_id: key.attempt_id.clone(),
            capability_digest: capability.digest(),
        },
    );
    (run, key)
}

fn bound_run(run_id: &str, attempt_id: &str) -> (WorkflowRunSnapshot, AttemptKey) {
    let (run, key) = dispatching_run(run_id, attempt_id);
    let run = apply(
        run,
        WorkflowEventPayload::AttemptBound {
            node_id: key.node_id.clone(),
            attempt_id: key.attempt_id.clone(),
            agent: QualifiedAddress::local("worker"),
        },
    );
    (run, key)
}

fn running_attempt_run(run_id: &str, attempt_id: &str) -> (WorkflowRunSnapshot, AttemptKey) {
    let (run, key) = bound_run(run_id, attempt_id);
    let run = apply(
        run,
        WorkflowEventPayload::AttemptRunning {
            node_id: key.node_id.clone(),
            attempt_id: key.attempt_id.clone(),
        },
    );
    (run, key)
}

fn cancelling_dispatch_run(run_id: &str, attempt_id: &str) -> (WorkflowRunSnapshot, AttemptKey) {
    let (run, key) = dispatching_run(run_id, attempt_id);
    let run = apply(
        run,
        WorkflowEventPayload::CancelRequested {
            reason: Some("cancel test".into()),
        },
    );
    (run, key)
}

fn terminal_run(run_id: &str, attempt_id: &str) -> (WorkflowRunSnapshot, AttemptKey) {
    let (run, key) = dispatching_run(run_id, attempt_id);
    let run = apply(
        run,
        WorkflowEventPayload::AttemptFailed {
            node_id: key.node_id.clone(),
            attempt_id: key.attempt_id.clone(),
            completion: AgentCompletion::new("terminal", None),
            failure: WorkflowAttemptFailure {
                class: WorkflowFailureClass::Permanent,
                reason: "terminal test".into(),
            },
        },
    );
    (run, key)
}
