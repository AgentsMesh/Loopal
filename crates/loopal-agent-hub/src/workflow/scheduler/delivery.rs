use std::sync::Arc;

use super::{WorkflowPreparedWorker, WorkflowSpawnFailure, WorkflowSpawner, bounded_shutdown};

pub(in crate::workflow) struct WorkflowPreparedDelivery {
    result: Option<Result<WorkflowPreparedWorker, WorkflowSpawnFailure>>,
    spawner: Arc<dyn WorkflowSpawner>,
}

impl WorkflowPreparedDelivery {
    pub(in crate::workflow) fn new(
        result: Result<WorkflowPreparedWorker, WorkflowSpawnFailure>,
        spawner: Arc<dyn WorkflowSpawner>,
    ) -> Self {
        Self {
            result: Some(result),
            spawner,
        }
    }

    pub(in crate::workflow) fn into_result(
        mut self,
    ) -> Result<WorkflowPreparedWorker, WorkflowSpawnFailure> {
        self.result
            .take()
            .expect("workflow preparation delivery is consumed once")
    }
}

impl Drop for WorkflowPreparedDelivery {
    fn drop(&mut self) {
        let Some(Ok(worker)) = self.result.take() else {
            return;
        };
        let spawner = self.spawner.clone();
        tokio::spawn(async move {
            bounded_shutdown(spawner, &worker.execution).await;
        });
    }
}
