use tokio::io::AsyncWriteExt;

use loopal_agent_client::test_support::drain_to_tracing;

#[tokio::test]
async fn drain_consumes_all_lines_without_blocking() {
    let (read_half, mut write_half) = tokio::io::duplex(1024);
    write_half
        .write_all(b"error line 1\nerror line 2\n")
        .await
        .unwrap();
    drop(write_half);
    drain_to_tracing(read_half).await;
}

#[tokio::test]
async fn drain_skips_empty_and_blank_lines() {
    let (read_half, mut write_half) = tokio::io::duplex(1024);
    write_half.write_all(b"\n  \nreal error\n\n").await.unwrap();
    drop(write_half);
    drain_to_tracing(read_half).await;
}
