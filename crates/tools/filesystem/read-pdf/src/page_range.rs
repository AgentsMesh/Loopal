pub fn parse_page_range(spec: &str, total: usize) -> Result<Vec<usize>, String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Err("empty page range".into());
    }

    if let Some((start_s, end_s)) = spec.split_once('-') {
        let start: usize = start_s
            .trim()
            .parse()
            .map_err(|_| format!("invalid page number: '{}'", start_s.trim()))?;
        let end: usize = end_s
            .trim()
            .parse()
            .map_err(|_| format!("invalid page number: '{}'", end_s.trim()))?;

        if start == 0 || end == 0 {
            return Err("page numbers are 1-based".into());
        }
        if start > end {
            return Err(format!("invalid range: start ({start}) > end ({end})"));
        }
        if start > total {
            return Err(format!("page {start} exceeds total pages ({total})"));
        }
        let end = end.min(total);
        Ok((start - 1..end).collect())
    } else {
        let page: usize = spec
            .parse()
            .map_err(|_| format!("invalid page number: '{spec}'"))?;
        if page == 0 {
            return Err("page numbers are 1-based".into());
        }
        if page > total {
            return Err(format!("page {page} exceeds total pages ({total})"));
        }
        Ok(vec![page - 1])
    }
}
