use std::sync::atomic::Ordering;

use super::try_recover_helpers::{
    Outcome, context_overflow_err, make_runner, ok_done, prefill_rejection_err, server_block_err,
};

#[tokio::test]
async fn server_block_error_triggers_one_retry() {
    let (mut runner, calls, mut rx) = make_runner(vec![
        Outcome::Err(server_block_err()),
        Outcome::Stream(ok_done()),
    ]);
    tokio::spawn(async move { while rx.recv().await.is_some() {} });
    let _ = runner.run().await.unwrap();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "expect retry after server-block"
    );
}

#[tokio::test]
async fn server_block_error_not_retried_twice() {
    let (mut runner, calls, mut rx) = make_runner(vec![
        Outcome::Err(server_block_err()),
        Outcome::Err(server_block_err()),
    ]);
    tokio::spawn(async move { while rx.recv().await.is_some() {} });
    let _ = runner.run().await.unwrap();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "second server-block must NOT trigger retry; expect 2 calls then error transition"
    );
}

#[tokio::test]
async fn context_overflow_triggers_one_retry() {
    let (mut runner, calls, mut rx) = make_runner(vec![
        Outcome::Err(context_overflow_err()),
        Outcome::Stream(ok_done()),
    ]);
    tokio::spawn(async move { while rx.recv().await.is_some() {} });
    let _ = runner.run().await.unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn context_overflow_not_retried_twice() {
    let (mut runner, calls, mut rx) = make_runner(vec![
        Outcome::Err(context_overflow_err()),
        Outcome::Err(context_overflow_err()),
    ]);
    tokio::spawn(async move { while rx.recv().await.is_some() {} });
    let _ = runner.run().await.unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn prefill_rejection_does_not_retry() {
    let (mut runner, calls, mut rx) = make_runner(vec![Outcome::Err(prefill_rejection_err())]);
    tokio::spawn(async move { while rx.recv().await.is_some() {} });
    let _ = runner.run().await.unwrap();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "PrefillRejected has no recovery path → no retry"
    );
}
