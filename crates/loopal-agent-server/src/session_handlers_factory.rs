use std::sync::Arc;

use loopal_config::ResolvedConfig;
use loopal_decision_api::DecisionMode;
use loopal_kernel::Kernel;
use loopal_provider_api::{ModelRouterReader, Provider, ProviderResolver, TaskType};
use loopal_runtime::frontend::permission_handler::PermissionHandler;
use loopal_runtime::frontend::question_handler::QuestionHandler;
use loopal_runtime::frontend::traits::EventEmitter;
use loopal_runtime::frontend::{
    ClassifierPermissionHandler, ClassifierQuestionHandler, DecisionCell, DecisionContext,
};

use crate::hub_broadcaster::HubBroadcaster;
use crate::ipc_handlers::{IpcPermissionHandler, IpcQuestionHandler, SessionRef};

struct KernelProviderResolver {
    kernel: Arc<Kernel>,
    router: ModelRouterReader,
}

impl ProviderResolver for KernelProviderResolver {
    fn resolve_for(
        &self,
        task: TaskType,
    ) -> Result<(String, Arc<dyn Provider>), loopal_error::LoopalError> {
        self.kernel.resolve_task(&self.router.read(), task)
    }
}

pub fn build_session_handlers(
    config: &ResolvedConfig,
    kernel: &Arc<Kernel>,
    session: SessionRef,
    context: DecisionContext,
    router: ModelRouterReader,
) -> (
    Box<dyn PermissionHandler>,
    Box<dyn QuestionHandler>,
    DecisionCell,
) {
    let emitter: Arc<dyn EventEmitter> = Arc::new(HubBroadcaster::new(session.clone(), None));
    build_session_handlers_with_emitter(config, kernel, session, context, router, emitter)
}

pub(crate) fn build_session_handlers_with_emitter(
    config: &ResolvedConfig,
    kernel: &Arc<Kernel>,
    session: SessionRef,
    context: DecisionContext,
    router: ModelRouterReader,
    emitter: Arc<dyn EventEmitter>,
) -> (
    Box<dyn PermissionHandler>,
    Box<dyn QuestionHandler>,
    DecisionCell,
) {
    let ipc_perm: Box<dyn PermissionHandler> = Box::new(IpcPermissionHandler::new(session.clone()));
    let ipc_q_arc: Arc<dyn QuestionHandler> = Arc::new(IpcQuestionHandler::new(session.clone()));
    if config.settings.decision_mode == DecisionMode::Agent {
        tracing::warn!("DecisionMode::Agent is not yet implemented; falling back to Classifier");
    }
    // Always build the classifier-wrapped chain; the DecisionCell decides at
    // request time whether to run the classifier or delegate to the IPC
    // fallback (Manual). This lets /decision flip modes mid-session without
    // rebuilding the handler chain.
    let decision = DecisionCell::new(config.settings.decision_mode);
    let classifier = Arc::new(
        loopal_classifier::ClassifierEngine::new_with_thresholds(
            config.instructions.clone(),
            config.settings.harness.cb_max_consecutive_denials,
            config.settings.harness.cb_max_total_denials,
        )
        .with_timeout(std::time::Duration::from_secs(
            config.settings.harness.classifier_timeout_secs,
        ))
        .with_question_system_prompt(config.classifier_prompt.clone()),
    );
    let resolver: Arc<dyn ProviderResolver> = Arc::new(KernelProviderResolver {
        kernel: kernel.clone(),
        router,
    });
    let auto_perm: Box<dyn PermissionHandler> = Box::new(
        ClassifierPermissionHandler::new(
            classifier.clone(),
            ipc_perm,
            resolver.clone(),
            context.clone(),
        )
        .with_decision(decision.clone()),
    );
    let auto_q: Box<dyn QuestionHandler> = Box::new(
        ClassifierQuestionHandler::new(classifier, ipc_q_arc, resolver, context, emitter)
            .with_decision(decision.clone()),
    );
    (auto_perm, auto_q, decision)
}
