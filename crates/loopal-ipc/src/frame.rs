use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt};

use loopal_error::LoopalError;

pub const MAX_IPC_FRAME_BYTES: usize = 64 * 1024 * 1024;

pub async fn read_frame(
    reader: &mut (impl AsyncBufRead + Unpin + ?Sized),
) -> Result<Option<Vec<u8>>, LoopalError> {
    read_frame_with_limit(reader, MAX_IPC_FRAME_BYTES).await
}

async fn read_frame_with_limit(
    reader: &mut (impl AsyncBufRead + Unpin + ?Sized),
    limit: usize,
) -> Result<Option<Vec<u8>>, LoopalError> {
    loop {
        let mut frame = Vec::new();
        let mut bounded = reader.take((limit + 1) as u64);
        let count = bounded
            .read_until(b'\n', &mut frame)
            .await
            .map_err(|error| LoopalError::Ipc(format!("read failed: {error}")))?;
        if count == 0 {
            return Ok(None);
        }
        if frame.last() != Some(&b'\n') {
            return Err(LoopalError::Ipc(format!("IPC frame exceeds {limit} bytes")));
        }
        frame.pop();
        let start = frame.iter().position(|byte| !byte.is_ascii_whitespace());
        let Some(start) = start else { continue };
        let end = frame
            .iter()
            .rposition(|byte| !byte.is_ascii_whitespace())
            .unwrap()
            + 1;
        return Ok(Some(frame[start..end].to_vec()));
    }
}

pub fn validate_outgoing_frame(data: &[u8]) -> Result<(), LoopalError> {
    if data.len() > MAX_IPC_FRAME_BYTES {
        return Err(LoopalError::Ipc(format!(
            "IPC frame exceeds {MAX_IPC_FRAME_BYTES} bytes"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufReader;

    #[tokio::test]
    async fn bounds_frames_before_unbounded_allocation() {
        let mut valid = BufReader::new(&b"  {\"ok\":true}  \n"[..]);
        assert_eq!(
            read_frame_with_limit(&mut valid, 32)
                .await
                .unwrap()
                .unwrap(),
            b"{\"ok\":true}"
        );

        let mut oversized = BufReader::new(&b"123456789\n"[..]);
        let error = read_frame_with_limit(&mut oversized, 8).await.unwrap_err();
        assert!(error.to_string().contains("exceeds 8 bytes"));

        let mut exact = BufReader::new(&b"12345678\n"[..]);
        assert_eq!(
            read_frame_with_limit(&mut exact, 8).await.unwrap().unwrap(),
            b"12345678"
        );
    }
}
