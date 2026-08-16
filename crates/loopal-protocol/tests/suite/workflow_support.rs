use loopal_protocol::*;

pub struct AcceptJson;

impl WorkflowJsonValidator for AcceptJson {
    type Error = String;

    fn validate(
        &self,
        _schema: &serde_json::Value,
        _value: &serde_json::Value,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub struct RejectJson;

impl WorkflowJsonValidator for RejectJson {
    type Error = String;

    fn validate(
        &self,
        _schema: &serde_json::Value,
        _value: &serde_json::Value,
    ) -> Result<(), Self::Error> {
        Err("schema mismatch".into())
    }
}

pub fn limits() -> WorkflowLimits {
    WorkflowLimits {
        max_nodes: 8,
        max_parallel: 2,
        max_attempts: 8,
        run_deadline_ms: 60_000,
        attempt_timeout_ms: 30_000,
        max_output_bytes: 4_096,
    }
}

pub fn node(id: &str, dependencies: &[&str]) -> WorkflowAgentNode {
    WorkflowAgentNode {
        id: id.into(),
        dependencies: dependencies.iter().copied().map(Into::into).collect(),
        task: format!("complete {id}"),
        worker_profile: WorkflowWorkerProfileRef::new("default"),
    }
}

pub fn text_spec() -> WorkflowSpec {
    WorkflowSpec {
        version: WORKFLOW_SPEC_V1,
        run_goal: "complete the workflow".into(),
        nodes: vec![node("source", &[]), node("output", &["source"])],
        limits: limits(),
        output_node: "output".into(),
        output_contract: WorkflowOutputContract::Text { max_bytes: 1_024 },
    }
}

pub fn json_spec() -> WorkflowSpec {
    WorkflowSpec {
        output_contract: WorkflowOutputContract::Json {
            max_bytes: 1_024,
            schema: serde_json::json!({
                "type": "object",
                "properties": {"answer": {"type": "integer"}},
                "required": ["answer"]
            }),
        },
        ..text_spec()
    }
}

pub fn planned(spec: WorkflowSpec) -> WorkflowRunSnapshot {
    WorkflowRunSnapshot::planned(
        WorkflowRunId::new("wrun_test"),
        QualifiedAddress::local("root"),
        spec,
        100,
    )
}

pub fn event(run: &WorkflowRunSnapshot, payload: WorkflowEventPayload) -> WorkflowEvent {
    WorkflowEvent {
        run_id: run.id.clone(),
        revision: run.revision + 1,
        occurred_at_unix_ms: run.updated_at_unix_ms + 1,
        payload,
    }
}

pub fn apply(run: &WorkflowRunSnapshot, payload: WorkflowEventPayload) -> WorkflowRunSnapshot {
    let event = event(run, payload);
    let WorkflowReduceOutcome::Applied(next) =
        reduce_workflow_event(run, &event, &AcceptJson).unwrap()
    else {
        panic!("fresh event was ignored")
    };
    *next
}

pub fn running(spec: WorkflowSpec) -> WorkflowRunSnapshot {
    let run = planned(spec);
    let run = apply(&run, WorkflowEventPayload::SpecValidated);
    apply(&run, WorkflowEventPayload::RunStarted)
}

pub fn dispatch(run: &WorkflowRunSnapshot, node: &str, attempt: &str) -> WorkflowRunSnapshot {
    apply(
        run,
        WorkflowEventPayload::DispatchIntended {
            node_id: node.into(),
            attempt_id: attempt.into(),
            capability_digest: WorkflowAttemptCapability::parse("33".repeat(32))
                .unwrap()
                .digest(),
        },
    )
}

pub fn bind_and_run(run: &WorkflowRunSnapshot, node: &str, attempt: &str) -> WorkflowRunSnapshot {
    let run = apply(
        run,
        WorkflowEventPayload::AttemptBound {
            node_id: node.into(),
            attempt_id: attempt.into(),
            agent: QualifiedAddress::local(format!("worker-{node}")),
        },
    );
    apply(
        &run,
        WorkflowEventPayload::AttemptRunning {
            node_id: node.into(),
            attempt_id: attempt.into(),
        },
    )
}

pub fn succeed(
    run: &WorkflowRunSnapshot,
    node: &str,
    attempt: &str,
    output: Option<WorkflowOutput>,
) -> WorkflowRunSnapshot {
    apply(
        run,
        WorkflowEventPayload::AttemptSucceeded {
            node_id: node.into(),
            attempt_id: attempt.into(),
            completion: AgentCompletion::goal(Some("done".into())),
            output,
        },
    )
}
