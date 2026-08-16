use std::sync::Arc;

use loopal_error::McpError;
use loopal_secret_client::SecretString;
use loopal_secret_runtime::Redactor;
use rmcp::model::{CallToolResult, RawContent, ResourceContents};

use crate::oauth_credential_seed::{OAUTH_RESPONSE_DENIED, OAuthCredentialSeed};

pub const BINARY_DENIED_MARKER: &str = "[MCP binary content denied]";

pub struct CallResultSanitizer {
    redactor: Redactor,
    oauth_credentials: Option<Arc<OAuthCredentialSeed>>,
}

impl CallResultSanitizer {
    pub fn new(seed: &[(String, SecretString)]) -> Self {
        Self {
            redactor: Redactor::from_pairs(seed),
            oauth_credentials: None,
        }
    }

    pub(crate) fn with_oauth_credentials(
        seed: &[(String, SecretString)],
        oauth_credentials: Arc<OAuthCredentialSeed>,
    ) -> Self {
        Self {
            redactor: Redactor::from_pairs(seed),
            oauth_credentials: Some(oauth_credentials),
        }
    }

    pub fn sanitize(&self, mut result: CallToolResult) -> CallToolResult {
        for content in &mut result.content {
            content.annotations = None;
            match &mut content.raw {
                RawContent::Text(text) => {
                    text.meta = None;
                    self.redact(&mut text.text);
                }
                RawContent::Resource(resource) => {
                    resource.meta = None;
                    match &mut resource.resource {
                        ResourceContents::TextResourceContents {
                            uri, text, meta, ..
                        } => {
                            if resource_uri_embeds_content(uri) {
                                *content = rmcp::model::Content::text(BINARY_DENIED_MARKER);
                            } else {
                                *meta = None;
                                self.redact(uri);
                                self.redact(text);
                            }
                        }
                        ResourceContents::BlobResourceContents { .. } => {
                            *content = rmcp::model::Content::text(BINARY_DENIED_MARKER);
                        }
                    }
                }
                RawContent::Image(_) | RawContent::Audio(_) => {
                    *content = rmcp::model::Content::text(BINARY_DENIED_MARKER);
                }
                RawContent::ResourceLink(link) => {
                    if resource_uri_embeds_content(&link.uri) {
                        *content = rmcp::model::Content::text(BINARY_DENIED_MARKER);
                    } else {
                        link.meta = None;
                        link.icons = None;
                        self.redact(&mut link.uri);
                        self.redact(&mut link.name);
                        if let Some(value) = &mut link.title {
                            self.redact(value);
                        }
                        if let Some(value) = &mut link.description {
                            self.redact(value);
                        }
                        if let Some(value) = &mut link.mime_type {
                            self.redact(value);
                        }
                    }
                }
            }
        }
        result.structured_content = None;
        result.meta = None;
        result
    }

    pub fn sanitize_text(&self, text: &str) -> String {
        self.redacted(text)
    }

    pub fn sanitize_json(&self, value: &mut serde_json::Value) {
        match value {
            serde_json::Value::String(text) => self.redact(text),
            serde_json::Value::Array(values) => {
                values
                    .iter_mut()
                    .for_each(|value| self.sanitize_json(value));
            }
            serde_json::Value::Object(values) => {
                let mut sanitized = serde_json::Map::new();
                for (key, mut value) in std::mem::take(values) {
                    self.sanitize_json(&mut value);
                    sanitized.insert(self.redacted(&key), value);
                }
                *values = sanitized;
            }
            _ => {}
        }
    }

    pub fn reject_blob(&self) -> Result<(), McpError> {
        Err(McpError::Protocol(BINARY_DENIED_MARKER.into()))
    }

    fn redact(&self, value: &mut String) {
        *value = self.redacted(value);
    }

    fn redacted(&self, value: &str) -> String {
        let value = self.redactor.scan_and_redact(value).0;
        let Some(credentials) = &self.oauth_credentials else {
            return value;
        };
        credentials
            .redactor()
            .map(|redactor| redactor.scan_and_redact(&value).0)
            .unwrap_or_else(|_| OAUTH_RESPONSE_DENIED.into())
    }
}

pub(crate) fn resource_uri_embeds_content(uri: &str) -> bool {
    uri.trim_start().split_once(':').is_some_and(|(scheme, _)| {
        scheme.eq_ignore_ascii_case("data") || scheme.eq_ignore_ascii_case("blob")
    })
}
