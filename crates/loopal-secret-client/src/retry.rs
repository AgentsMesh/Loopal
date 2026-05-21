use std::path::PathBuf;
use std::time::Duration;

use loopal_ipc::rpc_error::RpcError;
use loopal_protocol::SecretIpcError;

use crate::error::{SecretError, SecretResult};

#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
}

impl RetryPolicy {
    pub const fn new(max_attempts: u32, base_delay: Duration) -> Self {
        Self {
            max_attempts,
            base_delay,
        }
    }

    pub const fn default_ipc() -> Self {
        Self::new(3, Duration::from_millis(200))
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::default_ipc()
    }
}

/// Retry on `Ipc(_)` errors with exponential backoff. Permanent errors
/// propagate immediately — retrying them wastes time and hides the
/// real failure.
pub(crate) async fn retry_transient<F, Fut, T>(
    policy: RetryPolicy,
    mut op: F,
) -> SecretResult<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = SecretResult<T>>,
{
    let mut last_err: Option<SecretError> = None;
    for attempt in 1..=policy.max_attempts {
        match op().await {
            Ok(v) => return Ok(v),
            Err(e) if matches!(e, SecretError::Ipc(_)) && attempt < policy.max_attempts => {
                let delay = policy.base_delay * (1 << (attempt - 1));
                tokio::time::sleep(delay).await;
                last_err = Some(e);
            }
            Err(e) => return Err(e),
        }
    }
    Err(last_err.unwrap_or(SecretError::Ipc("retry exhausted".into())))
}

/// Decode a Hub RPC error into a typed `SecretError`. Reads the structured
/// `SecretIpcError` JSON from `RpcError::Remote { message }` — Hub-side
/// `secret_handlers::map_err` is the one writer of that field. Transport
/// failures map to `SecretError::Ipc` so retry_transient retries them.
pub(crate) fn classify_rpc(err: &RpcError) -> SecretError {
    if let RpcError::Remote { message, .. } = err {
        if let Ok(ipc) = serde_json::from_str::<SecretIpcError>(message) {
            return match ipc {
                SecretIpcError::SecretNotFound { name } => SecretError::SecretNotFound(name),
                SecretIpcError::VaultNotFound { cwd } => {
                    SecretError::VaultNotFound(PathBuf::from(cwd))
                }
                SecretIpcError::PermissionDenied => SecretError::PermissionDenied,
                SecretIpcError::DecryptFailed { detail } => SecretError::DecryptFailed(detail),
                SecretIpcError::InvalidName { name } => SecretError::InvalidName(name),
                SecretIpcError::TemplateParse { detail } => SecretError::TemplateParse(detail),
                SecretIpcError::Ipc { detail } => SecretError::Ipc(detail),
            };
        }
        // Hub returned unstructured Remote message — treat as transient IPC
        // so retry_transient gets another shot; never silently bucket into a
        // permanent error type.
        return SecretError::Ipc(message.clone());
    }
    SecretError::Ipc(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn succeeds_after_transient_failures() {
        let attempts = AtomicU32::new(0);
        let result = retry_transient(RetryPolicy::default(), || async {
            let n = attempts.fetch_add(1, Ordering::SeqCst);
            if n < 2 {
                Err(SecretError::Ipc(format!("transient #{n}")))
            } else {
                Ok::<u32, SecretError>(42)
            }
        })
        .await
        .unwrap();
        assert_eq!(result, 42);
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn propagates_permanent_immediately() {
        let attempts = AtomicU32::new(0);
        let result = retry_transient(RetryPolicy::default(), || async {
            attempts.fetch_add(1, Ordering::SeqCst);
            Err::<(), _>(SecretError::SecretNotFound("k".into()))
        })
        .await
        .unwrap_err();
        assert!(matches!(result, SecretError::SecretNotFound(_)));
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn exhausts_after_max_attempts() {
        let attempts = AtomicU32::new(0);
        let result = retry_transient(RetryPolicy::default(), || async {
            attempts.fetch_add(1, Ordering::SeqCst);
            Err::<(), _>(SecretError::Ipc("always fails".into()))
        })
        .await
        .unwrap_err();
        assert!(matches!(result, SecretError::Ipc(_)));
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn classify_decodes_structured_secret_not_found() {
        let payload = serde_json::to_string(&SecretIpcError::SecretNotFound {
            name: "api_key".into(),
        })
        .unwrap();
        let err = RpcError::Remote {
            code: -32600,
            message: payload,
            data: None,
        };
        assert!(matches!(
            classify_rpc(&err),
            SecretError::SecretNotFound(n) if n == "api_key"
        ));
    }

    #[test]
    fn classify_decodes_structured_permission_denied() {
        let payload = serde_json::to_string(&SecretIpcError::PermissionDenied).unwrap();
        let err = RpcError::Remote {
            code: -32600,
            message: payload,
            data: None,
        };
        assert!(matches!(classify_rpc(&err), SecretError::PermissionDenied));
    }

    #[test]
    fn classify_unstructured_remote_buckets_into_ipc() {
        let err = RpcError::Remote {
            code: -32603,
            message: "legacy plain-text error".into(),
            data: None,
        };
        assert!(matches!(classify_rpc(&err), SecretError::Ipc(_)));
    }

    #[test]
    fn classify_transport_buckets_into_ipc() {
        let err = RpcError::Transport("connection reset".into());
        assert!(matches!(classify_rpc(&err), SecretError::Ipc(_)));
    }
}
