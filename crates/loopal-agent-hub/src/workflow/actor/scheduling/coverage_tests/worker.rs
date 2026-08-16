fn active(
    owner: &WorkflowOwner,
    key: &AttemptKey,
    execution: AgentExecutionRef,
    phase: ActiveAttemptPhase,
) -> ActiveAttempt {
    ActiveAttempt {
        owner: owner.clone(),
        key: key.clone(),
        execution,
        outcome: None,
        outcome_waiter: None,
        shutdown_waiter: None,
        deadline_unix_ms: 100,
        shutdown_after_unix_ms: None,
        phase,
        stop: None,
    }
}

fn prepared_worker(
    execution: AgentExecutionRef,
) -> (
    WorkflowPreparedWorker,
    oneshot::Sender<WorkflowWorkerOutcome>,
) {
    let (outcome, receiver) = oneshot::channel();
    (
        WorkflowPreparedWorker {
            execution,
            outcome: receiver,
        },
        outcome,
    )
}

fn spawn_failure(class: WorkflowFailureClass, reason: &str) -> WorkflowSpawnFailure {
    WorkflowSpawnFailure {
        completion: AgentCompletion::new("scheduler_test_failure", None),
        failure: WorkflowAttemptFailure {
            class,
            reason: reason.into(),
        },
    }
}

fn reconnect_request(run: &WorkflowRunSnapshot, key: &AttemptKey) -> WorkflowAttemptReconnect {
    WorkflowAttemptReconnect {
        causation: WorkflowPermissionCausation {
            run_id: key.run_id.clone(),
            node_id: key.node_id.clone(),
            attempt_id: key.attempt_id.clone(),
        },
        capability: WorkflowAttemptCapability::parse("44".repeat(32)).unwrap(),
        execution: AgentExecutionRef::local(
            run.attempts[0]
                .agent
                .as_ref()
                .map_or("worker", |agent| agent.agent.as_str()),
            7,
        ),
    }
}
