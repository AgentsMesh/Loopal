use std::sync::Mutex;

use loopal_secret_client::SecretString;
use loopal_secret_runtime::Redactor;
use zeroize::Zeroizing;

pub(crate) const OAUTH_CREDENTIAL_ERROR: &str = "MCP OAuth credential boundary unavailable";
pub(crate) const OAUTH_RESPONSE_DENIED: &str = "[MCP OAuth response denied]";

#[derive(Default)]
pub(crate) struct OAuthCredentialSeed {
    state: Mutex<CredentialState>,
}

#[derive(Default)]
struct CredentialState {
    inactive: bool,
    tokens: Vec<Zeroizing<String>>,
}

impl OAuthCredentialSeed {
    pub(crate) fn observe(&self, token: Option<&str>) -> Result<(), &'static str> {
        let Some(token) = token.filter(|value| !value.is_empty()) else {
            return Ok(());
        };
        let mut state = self.state.lock().map_err(|_| OAUTH_CREDENTIAL_ERROR)?;
        if state.inactive {
            return Err(OAUTH_CREDENTIAL_ERROR);
        }
        if !state.tokens.iter().any(|known| known.as_str() == token) {
            state.tokens.push(Zeroizing::new(token.to_owned()));
        }
        Ok(())
    }

    pub(crate) fn redactor(&self) -> Result<Redactor, &'static str> {
        let state = self.state.lock().map_err(|_| OAUTH_CREDENTIAL_ERROR)?;
        if state.inactive {
            return Err(OAUTH_CREDENTIAL_ERROR);
        }
        let mut seed = Vec::with_capacity(state.tokens.len() * 2);
        for token in &state.tokens {
            seed.push((
                "mcp_oauth_bearer".into(),
                SecretString::from(format!("Bearer {}", token.as_str())),
            ));
            seed.push((
                "mcp_oauth_access_token".into(),
                SecretString::from(token.as_str().to_owned()),
            ));
        }
        Ok(Redactor::from_pairs(&seed))
    }

    pub(crate) fn deactivate(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.tokens.clear();
        state.inactive = true;
    }
}

#[cfg(test)]
#[path = "oauth_credential_seed_tests.rs"]
mod tests;
