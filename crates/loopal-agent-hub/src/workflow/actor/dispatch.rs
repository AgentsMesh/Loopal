use super::WorkflowCoordinator;
use crate::workflow::command::WorkflowCommand;

impl WorkflowCoordinator {
    pub(super) async fn dispatch(&mut self, command: WorkflowCommand) {
        match command {
            WorkflowCommand::Recover { owner, response } => {
                let _ = response.send(self.recover_owner(owner).await);
            }
            WorkflowCommand::Resume { owner, response } => {
                let _ = response.send(self.resume_owner(owner).await);
            }
            WorkflowCommand::Reconnect {
                owner,
                request,
                response,
            } => {
                let _ = response.send(self.reconnect_attempt(owner, request).await);
            }
            WorkflowCommand::WorkerHandshake {
                owner,
                request,
                response,
            } => {
                let _ = response.send(self.worker_handshake(owner, request).await);
            }
            WorkflowCommand::Snapshot { owner, response } => {
                let _ = response.send(self.snapshot(owner).await);
            }
            WorkflowCommand::Start {
                owner,
                request,
                response,
            } => match self.admit_start(owner.clone(), request).await {
                Err(error) => {
                    let _ = response.send(Err(error));
                }
                Ok(committed) => {
                    let run_id = committed.response.summary.id.clone();
                    let _ = response.send(Ok(committed.response));
                    if let Some(started) = committed.started {
                        self.publish_revision(&owner, &started);
                        if self.mode.executes()
                            && self.resumed_owners.contains(&owner)
                            && let Err(error) = self.admit_schedule(owner, run_id).await
                        {
                            tracing::error!(
                                %error,
                                "durable workflow start scheduling follow-up failed"
                            );
                        }
                    }
                }
            },
            WorkflowCommand::LookupStart {
                owner,
                request,
                response,
            } => {
                let _ = response.send(self.admit_lookup_start(owner, request).await);
            }
            WorkflowCommand::Get {
                owner,
                request,
                response,
            } => {
                let _ = response.send(self.admit_get(owner, request).await);
            }
            #[cfg(test)]
            WorkflowCommand::Schedule {
                owner,
                run_id,
                response,
            } => {
                let _ = response.send(self.admit_schedule(owner, run_id).await);
            }
            #[cfg(test)]
            WorkflowCommand::Pause { started, release } => {
                let _ = started.send(());
                let _ = release.await;
            }
            WorkflowCommand::WorkerPrepared {
                owner,
                key,
                prepared,
            } => {
                let _ = self.handle_prepared(owner, key, prepared).await;
            }
            WorkflowCommand::WorkerPreparationTimedOut {
                owner,
                key,
                failure,
            } => {
                self.handle_preparation_timed_out(owner, key, failure);
            }
            WorkflowCommand::WorkerPreparationAborted { owner, key, status } => {
                let _ = self.handle_preparation_aborted(owner, key, status).await;
            }
            WorkflowCommand::FinalizePreparationAbort { owner, key } => {
                let _ = self.handle_preparation_abort_settled(owner, key).await;
            }
            WorkflowCommand::PreparationDeliveryFinished { owner, key } => {
                let _ = self.handle_preparation_delivery_finished(owner, key).await;
            }
            WorkflowCommand::LatePreparationShutdown {
                owner,
                key,
                execution,
                status,
            } => {
                let _ = self
                    .handle_late_preparation_shutdown(owner, key, execution, status)
                    .await;
            }
            WorkflowCommand::WorkerActivated {
                owner,
                key,
                execution,
                result,
            } => {
                let _ = self.handle_activated(owner, key, execution, result).await;
            }
            WorkflowCommand::WorkerFinished {
                owner,
                key,
                execution,
                outcome,
            } => {
                let _ = self.handle_finished(owner, key, execution, outcome).await;
            }
            WorkflowCommand::WorkerOutcomeLost {
                owner,
                key,
                execution,
            } => {
                let _ = self.handle_outcome_lost(owner, key, execution).await;
            }
            WorkflowCommand::WorkerStopped {
                owner,
                key,
                execution,
                status,
            } => {
                let _ = self.handle_stopped(owner, key, execution, status).await;
            }
            WorkflowCommand::Cancel {
                owner,
                request,
                response,
            } => {
                let _ = response.send(self.admit_cancel(owner, request).await);
            }
            WorkflowCommand::Subscribe {
                owner,
                run_id,
                response,
            } => {
                let _ = response.send(self.subscribe(owner, run_id).await);
            }
            WorkflowCommand::Tick {
                now_unix_ms,
                response,
            } => {
                let _ = response.send(self.handle_tick(now_unix_ms).await);
            }
            WorkflowCommand::ActivateTerminalDeliveries { owner, response } => {
                let result = super::super::terminal_delivery::activate(self, owner).await;
                let _ = response.send(result);
            }
            WorkflowCommand::TerminalDeliveryResolved {
                owner,
                delivery_id,
                result,
                task_panicked,
            } => {
                if let Err(error) = super::super::terminal_delivery::resolved(
                    self,
                    owner,
                    delivery_id,
                    result,
                    task_panicked,
                )
                .await
                {
                    tracing::error!(%error, "workflow terminal delivery acknowledgement failed");
                }
            }
            WorkflowCommand::Shutdown { .. } => unreachable!("shutdown is handled by actor loop"),
        }
    }
}
