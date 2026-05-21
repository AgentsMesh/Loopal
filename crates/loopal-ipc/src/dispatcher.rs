use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use crate::jsonrpc;
use crate::rpc_error::RpcError;

#[async_trait]
pub trait RequestHandler: Send + Sync {
    async fn handle(&self, params: Value, ctx: &HandlerCtx) -> Result<Value, RpcError>;
}

pub struct HandlerCtx {
    pub from: String,
}

impl HandlerCtx {
    pub fn new(from: impl Into<String>) -> Self {
        Self { from: from.into() }
    }
}

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

type FallbackFn = Box<
    dyn for<'a> Fn(&'a str, Value, &'a HandlerCtx) -> BoxFuture<'a, Result<Value, RpcError>>
        + Send
        + Sync,
>;

struct FnHandler<F>
where
    F: for<'a> Fn(Value, &'a HandlerCtx) -> BoxFuture<'a, Result<Value, RpcError>>
        + Send
        + Sync
        + 'static,
{
    f: F,
}

#[async_trait]
impl<F> RequestHandler for FnHandler<F>
where
    F: for<'a> Fn(Value, &'a HandlerCtx) -> BoxFuture<'a, Result<Value, RpcError>>
        + Send
        + Sync
        + 'static,
{
    async fn handle(&self, params: Value, ctx: &HandlerCtx) -> Result<Value, RpcError> {
        (self.f)(params, ctx).await
    }
}

#[derive(Default)]
pub struct DispatcherBuilder {
    handlers: HashMap<&'static str, Arc<dyn RequestHandler>>,
    fallback: Option<FallbackFn>,
}

impl DispatcherBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<H>(mut self, method_name: &'static str, handler: H) -> Self
    where
        H: RequestHandler + 'static,
    {
        if self
            .handlers
            .insert(method_name, Arc::new(handler))
            .is_some()
        {
            panic!("duplicate handler registration for method: {method_name}");
        }
        self
    }

    pub fn register_fn<F>(self, method_name: &'static str, f: F) -> Self
    where
        F: for<'a> Fn(Value, &'a HandlerCtx) -> BoxFuture<'a, Result<Value, RpcError>>
            + Send
            + Sync
            + 'static,
    {
        self.register(method_name, FnHandler { f })
    }

    pub fn fallback<F>(mut self, f: F) -> Self
    where
        F: for<'a> Fn(&'a str, Value, &'a HandlerCtx) -> BoxFuture<'a, Result<Value, RpcError>>
            + Send
            + Sync
            + 'static,
    {
        self.fallback = Some(Box::new(f));
        self
    }

    pub fn build(self) -> Dispatcher {
        Dispatcher {
            handlers: self.handlers,
            fallback: self.fallback,
        }
    }
}

pub struct Dispatcher {
    handlers: HashMap<&'static str, Arc<dyn RequestHandler>>,
    fallback: Option<FallbackFn>,
}

impl Dispatcher {
    pub async fn dispatch(
        &self,
        method: &str,
        params: Value,
        ctx: &HandlerCtx,
    ) -> Result<Value, RpcError> {
        if let Some(h) = self.handlers.get(method) {
            return h.handle(params, ctx).await;
        }
        if let Some(ref f) = self.fallback {
            return f(method, params, ctx).await;
        }
        Err(RpcError::Remote {
            code: jsonrpc::METHOD_NOT_FOUND,
            message: format!("unknown method: {method}"),
            data: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn register_fn_then_dispatch() {
        let d = DispatcherBuilder::new()
            .register_fn("echo", |params, _ctx| Box::pin(async move { Ok(params) }))
            .build();
        let ctx = HandlerCtx::new("test");
        let out = d
            .dispatch("echo", serde_json::json!({"a": 1}), &ctx)
            .await
            .unwrap();
        assert_eq!(out["a"], 1);
    }

    #[tokio::test]
    async fn unknown_method_returns_method_not_found() {
        let d = DispatcherBuilder::new().build();
        let ctx = HandlerCtx::new("test");
        let err = d
            .dispatch("nope", serde_json::Value::Null, &ctx)
            .await
            .unwrap_err();
        match err {
            RpcError::Remote { code, .. } => assert_eq!(code, jsonrpc::METHOD_NOT_FOUND),
            _ => panic!("expected Remote METHOD_NOT_FOUND, got {err:?}"),
        }
    }

    #[tokio::test]
    async fn fallback_receives_method_name() {
        let d = DispatcherBuilder::new()
            .fallback(|method, _params, _ctx| {
                let method = method.to_string();
                Box::pin(async move { Ok(serde_json::json!({"method": method})) })
            })
            .build();
        let ctx = HandlerCtx::new("test");
        let out = d
            .dispatch("custom/x", serde_json::Value::Null, &ctx)
            .await
            .unwrap();
        assert_eq!(out["method"], "custom/x");
    }

    #[test]
    #[should_panic(expected = "duplicate handler registration")]
    fn duplicate_register_panics() {
        let _ = DispatcherBuilder::new()
            .register_fn("foo", |_p, _c| Box::pin(async { Ok(Value::Null) }))
            .register_fn("foo", |_p, _c| Box::pin(async { Ok(Value::Null) }));
    }
}
