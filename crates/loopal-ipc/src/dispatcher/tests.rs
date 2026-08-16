use super::*;

#[tokio::test]
async fn register_fn_then_dispatch() {
    let dispatcher = DispatcherBuilder::new()
        .register_fn("echo", |params, _ctx| Box::pin(async move { Ok(params) }))
        .build();
    let ctx = HandlerCtx::new("test");
    let output = dispatcher
        .dispatch("echo", serde_json::json!({"a": 1}), &ctx)
        .await
        .unwrap();
    assert_eq!(output["a"], 1);
}

#[test]
fn registered_method_snapshot_is_sorted_and_complete() {
    let dispatcher = DispatcherBuilder::new()
        .register_fn("z", |_params, _ctx| Box::pin(async { Ok(Value::Null) }))
        .register_fn("a", |_params, _ctx| Box::pin(async { Ok(Value::Null) }))
        .build();
    assert_eq!(dispatcher.registered_methods(), vec!["a", "z"]);
}

#[tokio::test]
async fn unknown_method_returns_method_not_found() {
    let dispatcher = DispatcherBuilder::new().build();
    let ctx = HandlerCtx::new("test");
    let error = dispatcher
        .dispatch("nope", serde_json::Value::Null, &ctx)
        .await
        .unwrap_err();
    match error {
        RpcError::Remote { code, .. } => assert_eq!(code, jsonrpc::METHOD_NOT_FOUND),
        _ => panic!("expected Remote METHOD_NOT_FOUND, got {error:?}"),
    }
}

#[tokio::test]
async fn fallback_receives_method_name() {
    let dispatcher = DispatcherBuilder::new()
        .fallback(|method, _params, _ctx| {
            let method = method.to_string();
            Box::pin(async move { Ok(serde_json::json!({"method": method})) })
        })
        .build();
    let ctx = HandlerCtx::new("test");
    let output = dispatcher
        .dispatch("custom/x", serde_json::Value::Null, &ctx)
        .await
        .unwrap();
    assert_eq!(output["method"], "custom/x");
}

#[test]
fn extension_is_typed_and_arc_backed() {
    let value = Arc::new(vec!["trusted"]);
    let ctx = HandlerCtx::new("test").with_extension(value.clone());

    assert!(Arc::ptr_eq(&ctx.extension::<Vec<&str>>().unwrap(), &value));
    assert!(ctx.extension::<String>().is_none());
}

#[test]
fn from_preserves_constructor_behavior() {
    let ctx = HandlerCtx::from("test");
    assert_eq!(ctx.from, "test");
    assert!(ctx.extension::<String>().is_none());
}

#[test]
#[should_panic(expected = "duplicate handler registration")]
fn duplicate_register_panics() {
    let _ = DispatcherBuilder::new()
        .register_fn("foo", |_params, _ctx| Box::pin(async { Ok(Value::Null) }))
        .register_fn("foo", |_params, _ctx| Box::pin(async { Ok(Value::Null) }));
}
