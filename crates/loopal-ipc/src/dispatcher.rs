use std::any::Any;
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
    extension: Option<Arc<dyn Any + Send + Sync>>,
}

impl HandlerCtx {
    pub fn new(from: impl Into<String>) -> Self {
        Self {
            from: from.into(),
            extension: None,
        }
    }

    pub fn with_extension<T>(mut self, extension: Arc<T>) -> Self
    where
        T: Any + Send + Sync,
    {
        self.extension = Some(extension);
        self
    }

    pub fn extension<T>(&self) -> Option<Arc<T>>
    where
        T: Any + Send + Sync,
    {
        self.extension.clone()?.downcast().ok()
    }
}

impl<T> From<T> for HandlerCtx
where
    T: Into<String>,
{
    fn from(from: T) -> Self {
        Self::new(from)
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
    pub fn registered_methods(&self) -> Vec<&'static str> {
        let mut methods: Vec<_> = self.handlers.keys().copied().collect();
        methods.sort_unstable();
        methods
    }

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
#[path = "dispatcher/tests.rs"]
mod tests;
