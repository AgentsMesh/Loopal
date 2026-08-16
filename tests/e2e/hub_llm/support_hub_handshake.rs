use tokio::io::{AsyncBufReadExt, BufReader};

pub(super) async fn find_handshake_line(
    stdout: tokio::process::ChildStdout,
) -> std::io::Result<String> {
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    loop {
        line.clear();
        let read = reader.read_line(&mut line).await?;
        if read == 0 {
            return Err(std::io::Error::other("hub stdout closed before handshake"));
        }
        if line.starts_with("LOOPAL_HUB ") {
            return Ok(line.trim_end().to_string());
        }
    }
}
