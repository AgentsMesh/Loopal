use loopal_error::McpError;

pub(crate) const REDACTED_STDERR: &str = "MCP server emitted redacted stderr";

pub(crate) fn endpoint_label(_raw: &str) -> &'static str {
    "mcp-http"
}

pub(crate) fn connection_failed(stage: &'static str) -> McpError {
    McpError::ConnectionFailed(stage.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_and_errors_exclude_credentials_paths_queries_and_fragments() {
        let marker = "mcp-diagnostic-secret-marker";
        let input = format!(
            "https://user:{marker}@example.test:8443/private/{marker}?token={marker}#{marker}"
        );
        let label = endpoint_label(&input);
        assert_eq!(label, "mcp-http");
        assert!(!label.contains(marker));
        let error = connection_failed("MCP HTTP connection failed");
        assert!(!format!("{error}").contains(marker));
        assert!(!format!("{error:?}").contains(marker));
        assert!(!REDACTED_STDERR.contains(marker));
    }
}
