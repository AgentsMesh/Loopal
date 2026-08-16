use std::io::{self, BufRead};

pub(crate) fn read_bounded_line(
    reader: &mut impl BufRead,
    buffer: &mut Vec<u8>,
    max_bytes: usize,
) -> io::Result<usize> {
    buffer.clear();
    let mut read = 0;
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return Ok(read);
        }
        let take = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |position| position + 1);
        let allowed = max_bytes.saturating_add(1).saturating_sub(buffer.len());
        buffer.extend_from_slice(&available[..take.min(allowed)]);
        let ended = take < available.len() || available[take - 1] == b'\n';
        reader.consume(take);
        read += take;
        if ended || buffer.len() > max_bytes {
            return Ok(read);
        }
    }
}
