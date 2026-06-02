use loopal_error::{LoopalError, Result};
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::{ErrorClass, default_classify_error};
use tracing::{info, warn};

use super::run::CONTEXT_OVERFLOW_BANNER;
use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    pub(super) fn classify_turn_error(&self, err: &LoopalError) -> ErrorClass {
        match self
            .params
            .deps
            .kernel
            .resolve_provider(self.params.config.model())
        {
            Ok(provider) => provider.classify_error(err),
            Err(_) => default_classify_error(err),
        }
    }

    pub(super) async fn try_recover(
        &mut self,
        class: ErrorClass,
        server_block_retry: &mut bool,
        context_overflow_retry: &mut bool,
    ) -> Result<bool> {
        match class {
            ErrorClass::ServerBlockError if !*server_block_retry => {
                *server_block_retry = true;
                info!("condensing server blocks after API rejection, retrying");
                self.turns
                    .with_wire_mut(loopal_context::condense_server_blocks);
                Ok(true)
            }
            ErrorClass::ContextOverflow if !*context_overflow_retry => {
                *context_overflow_retry = true;
                info!("context overflow detected, force-compacting and retrying");
                // Emit banner ONLY after compact succeeded — pre-fix showed
                // "retrying" then errored when force_compact silently bailed.
                let compacted = self.force_compact(None).await?;
                if !compacted {
                    warn!("force_compact declined; ContextOverflow propagates");
                    return Ok(false);
                }
                self.emit_cosmetic(AgentEventPayload::Error {
                    message: CONTEXT_OVERFLOW_BANNER.into(),
                })
                .await;
                Ok(true)
            }
            // PrefillRejected here means provider's finalize_messages let it
            // through — the model catalog has supports_prefill=true wrongly.
            // Fail fast: silent retry would loop without state change.
            _ => Ok(false),
        }
    }
}
