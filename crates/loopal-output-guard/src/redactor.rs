use std::collections::{HashMap, HashSet};

use aho_corasick::{AhoCorasick, MatchKind};
use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;

#[derive(Clone, PartialEq, Eq)]
pub struct Redaction<T> {
    value: T,
    secret_names: Vec<String>,
}

impl<T> Redaction<T> {
    pub(crate) fn new(value: T, secret_names: Vec<String>) -> Self {
        Self {
            value,
            secret_names,
        }
    }
}

macro_rules! redaction_api {
    ($value:ty) => {
        impl Redaction<$value> {
            pub fn into_inner(self) -> $value {
                self.value
            }

            pub fn secret_names(&self) -> &[String] {
                &self.secret_names
            }
        }

        impl std::fmt::Debug for Redaction<$value> {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter
                    .debug_struct("Redaction")
                    .field("secret_count", &self.secret_names.len())
                    .finish_non_exhaustive()
            }
        }
    };
}

redaction_api!(String);
redaction_api!(Vec<u8>);
redaction_api!(GuardedText);
redaction_api!(crate::value::GuardedJson);

#[derive(Clone, PartialEq, Eq)]
pub struct GuardedText(String);

impl std::fmt::Debug for GuardedText {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GuardedText")
            .field("byte_size", &self.0.len())
            .finish_non_exhaustive()
    }
}

impl GuardedText {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum OutputGuardError {
    #[error("redacted text is {actual_bytes} bytes; limit is {max_bytes} bytes")]
    ByteLimitExceeded {
        actual_bytes: usize,
        max_bytes: usize,
    },
}

pub struct OutputGuard {
    matcher: Option<AhoCorasick>,
    names: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("secret redactor could not be built")]
pub struct OutputGuardBuildError;

impl OutputGuard {
    pub fn new(seed: &[(String, SecretString)]) -> Result<Self, OutputGuardBuildError> {
        let mut by_plaintext = HashMap::new();
        for (name, secret) in seed {
            let plaintext = secret.expose_secret();
            if !plaintext.is_empty() {
                by_plaintext
                    .entry(plaintext.to_string())
                    .or_insert_with(|| name.clone());
            }
        }
        let mut patterns: Vec<(String, String)> = by_plaintext
            .into_iter()
            .map(|(plaintext, name)| (name, plaintext))
            .collect();
        patterns.sort_by(|left, right| right.1.len().cmp(&left.1.len()));
        if patterns.is_empty() {
            return Ok(Self {
                matcher: None,
                names: Vec::new(),
            });
        }
        let matcher = AhoCorasick::builder()
            .match_kind(MatchKind::LeftmostLongest)
            .build(patterns.iter().map(|(_, plaintext)| plaintext))
            .map_err(|_| OutputGuardBuildError)?;
        let names = patterns.into_iter().map(|(name, _)| name).collect();
        Ok(Self {
            matcher: Some(matcher),
            names,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.matcher.is_none()
    }

    pub fn redact_text(&self, input: &str) -> Redaction<String> {
        let Some(matcher) = &self.matcher else {
            return Redaction::new(input.to_string(), Vec::new());
        };
        let mut output = String::with_capacity(input.len());
        let mut names = Vec::new();
        let mut last_end = 0;
        for matched in matcher.find_iter(input) {
            output.push_str(&input[last_end..matched.start()]);
            let name = &self.names[matched.pattern().as_usize()];
            output.push_str("<secret_ref:");
            output.push_str(name);
            output.push('>');
            names.push(name.clone());
            last_end = matched.end();
        }
        output.push_str(&input[last_end..]);
        deduplicate(&mut names);
        Redaction::new(output, names)
    }

    pub fn guard_text(
        &self,
        input: &str,
        max_bytes: usize,
    ) -> Result<Redaction<GuardedText>, OutputGuardError> {
        let redacted = self.redact_text(input);
        let (value, names) = redacted.into_parts();
        if value.len() > max_bytes {
            return Err(OutputGuardError::ByteLimitExceeded {
                actual_bytes: value.len(),
                max_bytes,
            });
        }
        Ok(Redaction::new(GuardedText(value), names))
    }
}

impl Redaction<String> {
    pub(crate) fn into_parts(self) -> (String, Vec<String>) {
        (self.value, self.secret_names)
    }
}

pub(crate) fn deduplicate(names: &mut Vec<String>) {
    let mut seen = HashSet::new();
    names.retain(|name| seen.insert(name.clone()));
}
