use rmcp::transport::streamable_http_client::StreamableHttpError;

use super::sanitize_error;
use crate::oauth_credential_seed::OAUTH_RESPONSE_DENIED;

#[test]
fn preserves_only_non_textual_worker_control_errors() {
    assert!(matches!(
        sanitize_error::<reqwest::Error>(StreamableHttpError::ServerDoesNotSupportSse),
        StreamableHttpError::ServerDoesNotSupportSse
    ));
    assert!(matches!(
        sanitize_error::<reqwest::Error>(StreamableHttpError::ServerDoesNotSupportDeleteSession),
        StreamableHttpError::ServerDoesNotSupportDeleteSession
    ));
    assert!(matches!(
        sanitize_error::<reqwest::Error>(StreamableHttpError::SessionExpired),
        StreamableHttpError::SessionExpired
    ));
    assert!(
        sanitize_error::<reqwest::Error>(StreamableHttpError::UnexpectedEndOfStream)
            .to_string()
            .contains(OAUTH_RESPONSE_DENIED)
    );
}
