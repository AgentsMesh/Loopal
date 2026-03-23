/// Styled text wrapping — wrap a Line of mixed-style Spans at a given width.
///
/// Algorithm (flatten-slice):
/// 1. Flatten Spans → plain text + `Vec<(byte_range, Style)>`
/// 2. `textwrap::wrap(plain, width)` computes word-break points
/// 3. Slice span bounds by each wrapped line's byte range → rebuild Spans
use ratatui::prelude::*;

/// Wrap a `Line` containing mixed-style Spans to the given width.
///
/// Returns one `Line` per visual row, preserving per-character styles.
pub fn styled_wrap(line: &Line<'static>, width: u16) -> Vec<Line<'static>> {
    let w = (width as usize).max(1);

    // Step 1: flatten spans into plain text + style bounds
    let (flat, bounds) = flatten(line);

    if flat.is_empty() {
        return vec![Line::from("")];
    }

    // Step 2: word wrap on plain text
    let wrapped = textwrap::wrap(&flat, w);

    // Step 3: rebuild styled lines from byte ranges
    let mut result = Vec::with_capacity(wrapped.len());
    let mut byte_offset: usize = 0;

    for segment in &wrapped {
        let seg_str: &str = segment;
        // textwrap may strip leading/trailing whitespace; find actual position
        let seg_start = find_segment_start(&flat, byte_offset, seg_str);
        let seg_end = seg_start + seg_str.len();

        let spans = slice_spans(&flat, &bounds, seg_start, seg_end);
        result.push(Line::from(spans));

        byte_offset = seg_end;
    }

    if result.is_empty() {
        result.push(Line::from(""));
    }
    result
}

/// Flatten all spans into a single string and a parallel list of
/// `(byte_start, byte_end, Style)` entries.
fn flatten(line: &Line<'static>) -> (String, Vec<(usize, usize, Style)>) {
    let mut flat = String::new();
    let mut bounds = Vec::with_capacity(line.spans.len());

    for span in &line.spans {
        let start = flat.len();
        flat.push_str(&span.content);
        bounds.push((start, flat.len(), span.style));
    }
    (flat, bounds)
}

/// Find where `segment` starts in `flat` at or after `from`.
///
/// textwrap may skip whitespace between segments, so we search forward.
fn find_segment_start(flat: &str, from: usize, segment: &str) -> usize {
    if segment.is_empty() {
        return from;
    }
    // Fast path: exact position
    if flat[from..].starts_with(segment) {
        return from;
    }
    // Scan forward (textwrap skips whitespace)
    flat[from..]
        .find(segment)
        .map(|idx| from + idx)
        .unwrap_or(from)
}

/// Extract styled Spans for the byte range `[start, end)` from the
/// flattened text, splitting Spans at boundaries as needed.
fn slice_spans(
    flat: &str,
    bounds: &[(usize, usize, Style)],
    start: usize,
    end: usize,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for &(b_start, b_end, style) in bounds {
        // Skip spans entirely before or after the range
        if b_end <= start || b_start >= end {
            continue;
        }
        let s = b_start.max(start);
        let e = b_end.min(end);
        if s < e {
            spans.push(Span::styled(flat[s..e].to_string(), style));
        }
    }
    if spans.is_empty() {
        spans.push(Span::raw(""));
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_short_no_wrap() {
        let line = Line::from("hello");
        let result = styled_wrap(&line, 80);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_plain_long_wraps() {
        let text = "word ".repeat(20);
        let line = Line::from(text);
        let result = styled_wrap(&line, 30);
        assert!(result.len() > 1);
    }

    #[test]
    fn test_mixed_styles_preserved() {
        let line = Line::from(vec![
            Span::styled("bold ", Style::default().bold()),
            Span::raw("normal"),
        ]);
        let result = styled_wrap(&line, 80);
        assert_eq!(result.len(), 1);
        assert!(
            result[0].spans[0]
                .style
                .add_modifier
                .contains(Modifier::BOLD)
        );
    }
}
