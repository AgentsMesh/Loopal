use loopal_protocol::event_id::{
    TurnContext, current_correlation_id, next_event_id, propagate_to_spawn, scope_correlation,
    scope_turn,
};

#[tokio::test]
async fn try_current_is_none_outside_scope() {
    assert!(TurnContext::try_current().is_none());
}

#[tokio::test]
async fn try_current_inside_scope_yields_ctx() {
    scope_turn(42, async {
        let ctx = TurnContext::try_current().expect("in scope");
        assert_eq!(ctx.turn_id, 42);
        assert_eq!(ctx.correlation_id, 0);
    })
    .await;
    assert!(TurnContext::try_current().is_none());
}

#[tokio::test]
async fn scope_correlation_inherits_turn_id() {
    scope_turn(7, async {
        scope_correlation(99, async {
            let ctx = TurnContext::try_current().unwrap();
            assert_eq!(ctx.turn_id, 7);
            assert_eq!(ctx.correlation_id, 99);
        })
        .await;
        assert_eq!(current_correlation_id(), 0);
    })
    .await;
}

#[tokio::test]
async fn next_event_id_monotonic() {
    let a = next_event_id();
    let b = next_event_id();
    assert!(b > a);
}

#[tokio::test]
async fn current_or_default_no_panic_outside_scope() {
    let ctx = TurnContext::current_or_default();
    assert_eq!(ctx.turn_id, 0);
}

#[tokio::test]
#[should_panic(expected = "require_current called outside scope_turn")]
async fn require_current_panics_outside_scope() {
    let _ = TurnContext::require_current();
}

#[tokio::test]
async fn require_current_inside_scope_succeeds() {
    scope_turn(7, async {
        let ctx = TurnContext::require_current();
        assert_eq!(ctx.turn_id, 7);
    })
    .await;
}

#[tokio::test]
async fn propagate_to_spawn_carries_context_across_tokio_spawn() {
    let observed = scope_turn(123, async {
        scope_correlation(456, async {
            tokio::spawn(propagate_to_spawn(async move {
                // Inside spawned task — without propagate, this would be None.
                TurnContext::try_current().map(|c| (c.turn_id, c.correlation_id))
            }))
            .await
            .unwrap()
        })
        .await
    })
    .await;
    assert_eq!(observed, Some((123, 456)));
}

#[tokio::test]
async fn propagate_to_spawn_outside_scope_yields_no_scope() {
    // No parent scope_turn; spawned task should also have no scope.
    let observed = tokio::spawn(propagate_to_spawn(async { TurnContext::try_current() }))
        .await
        .unwrap();
    assert!(observed.is_none());
}
