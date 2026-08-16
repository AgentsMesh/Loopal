use std::sync::{Arc, RwLock};

use loopal_protocol::{AgentCompletion, AgentEvent};
use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;

use crate::{
    MAX_STREAM_SECRET_BYTES, MAX_STREAM_SECRET_NAME_BYTES, MAX_STREAM_SECRET_PATTERNS,
    MAX_STREAM_SECRET_TOTAL_BYTES,
};

#[derive(Clone, Default)]
pub struct FinalSinkRedactionSeed {
    entries: Arc<RwLock<Vec<(String, SecretString)>>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
#[error("final-sink redaction seed unavailable")]
pub struct FinalSinkRedactionSeedError;

impl FinalSinkRedactionSeed {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn observe(
        &self,
        name: impl Into<String>,
        value: SecretString,
    ) -> Result<(), FinalSinkRedactionSeedError> {
        let name = name.into();
        let plaintext = value.expose_secret();
        if plaintext.is_empty() {
            return Ok(());
        }
        if name.len() > MAX_STREAM_SECRET_NAME_BYTES || plaintext.len() > MAX_STREAM_SECRET_BYTES {
            return Err(FinalSinkRedactionSeedError);
        }

        let mut entries = self
            .entries
            .write()
            .map_err(|_| FinalSinkRedactionSeedError)?;
        if entries
            .iter()
            .any(|(_, existing)| existing.expose_secret() == plaintext)
        {
            return Ok(());
        }
        if entries.len() >= MAX_STREAM_SECRET_PATTERNS {
            return Err(FinalSinkRedactionSeedError);
        }
        let total_bytes = entries
            .iter()
            .try_fold(plaintext.len(), |total, (_, existing)| {
                total.checked_add(existing.expose_secret().len())
            })
            .ok_or(FinalSinkRedactionSeedError)?;
        if total_bytes > MAX_STREAM_SECRET_TOTAL_BYTES {
            return Err(FinalSinkRedactionSeedError);
        }
        entries.push((name, value));
        Ok(())
    }

    pub fn snapshot(&self) -> Result<Vec<(String, SecretString)>, FinalSinkRedactionSeedError> {
        self.entries
            .read()
            .map(|entries| entries.clone())
            .map_err(|_| FinalSinkRedactionSeedError)
    }

    pub fn guard_completion(&self, completion: AgentCompletion) -> AgentCompletion {
        self.guard_completion_with_result_limit(
            completion,
            crate::MAX_AGENT_COMPLETION_RESULT_BYTES,
        )
    }

    pub fn guard_completion_with_result_limit(
        &self,
        completion: AgentCompletion,
        max_result_bytes: usize,
    ) -> AgentCompletion {
        match self.snapshot() {
            Ok(snapshot) => crate::guard_or_reject_agent_completion_with_result_limit(
                completion,
                &snapshot,
                max_result_bytes,
            )
            .into_completion(),
            Err(_) => crate::rejected_agent_completion(),
        }
    }

    pub fn guard_event(&self, event: AgentEvent) -> AgentEvent {
        match self.snapshot() {
            Ok(snapshot) => crate::guard_or_reject_agent_event(event, &snapshot).into_event(),
            Err(_) => crate::rejected_agent_event(&event),
        }
    }
}

impl std::fmt::Debug for FinalSinkRedactionSeed {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let entry_count = self.entries.read().map(|entries| entries.len()).ok();
        formatter
            .debug_struct("FinalSinkRedactionSeed")
            .field("entry_count", &entry_count)
            .finish_non_exhaustive()
    }
}
