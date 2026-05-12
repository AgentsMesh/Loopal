use std::sync::Arc;

use loopal_config::ResolvedConfig;
use loopal_decision_api::DecisionMode;
use loopal_kernel::Kernel;
use loopal_provider_api::{ModelRouter, Provider, ProviderResolver, TaskType};
use loopal_runtime::frontend::permission_handler::PermissionHandler;
use loopal_runtime::frontend::question_handler::QuestionHandler;
use loopal_runtime::frontend::traits::EventEmitter;
use loopal_runtime::frontend::{
    ClassifierPermissionHandler, ClassifierQuestionHandler, DecisionContext,
};

use crate::hub_broadcaster::HubBroadcaster;
use crate::ipc_handlers::{IpcPermissionHandler, IpcQuestionHandler, SessionRef};

struct KernelProviderResolver {
    kernel: Arc<Kernel>,
    router: ModelRouter,
}

impl ProviderResolver for KernelProviderResolver {
    fn resolve_for(
        &self,
        task: TaskType,
    ) -> Result<(String, Arc<dyn Provider>), loopal_error::LoopalError> {
        let model = self.router.resolve(task).to_string();
        let provider = self.kernel.resolve_provider(&model)?;
        Ok((model, provider))
    }
}

pub fn build_session_handlers(
    config: &ResolvedConfig,
    kernel: &Arc<Kernel>,
    session: SessionRef,
    context: DecisionContext,
) -> (Box<dyn PermissionHandler>, Box<dyn QuestionHandler>) {
    let ipc_perm: Box<dyn PermissionHandler> = Box::new(IpcPermissionHandler::new(session.clone()));
    let ipc_q_arc: Arc<dyn QuestionHandler> = Arc::new(IpcQuestionHandler::new(session.clone()));
    match config.settings.decision_mode {
        DecisionMode::Manual => {
            let ipc_q: Box<dyn QuestionHandler> = Box::new(IpcQuestionHandler::new(session));
            return (ipc_perm, ipc_q);
        }
        DecisionMode::Classifier => {}
        DecisionMode::Agent => {
            tracing::warn!(
                "DecisionMode::Agent is not yet implemented; falling back to Classifier"
            );
        }
    }
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
    let router = ModelRouter::from_parts(
        config.settings.model.clone(),
        config.settings.model_routing.clone(),
    );
    let resolver: Arc<dyn ProviderResolver> = Arc::new(KernelProviderResolver {
        kernel: kernel.clone(),
        router,
    });
    let emitter: Arc<dyn EventEmitter> = Arc::new(HubBroadcaster::new(session, None));
    let auto_perm: Box<dyn PermissionHandler> = Box::new(ClassifierPermissionHandler::new(
        classifier.clone(),
        ipc_perm,
        resolver.clone(),
        context.clone(),
    ));
    let auto_q: Box<dyn QuestionHandler> = Box::new(ClassifierQuestionHandler::new(
        classifier, ipc_q_arc, resolver, context, emitter,
    ));
    (auto_perm, auto_q)
}
