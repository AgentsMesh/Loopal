use std::collections::BTreeMap;

pub fn fields<'a>(
    value: &'a str,
    min: usize,
    max: usize,
    line: usize,
    kind: &str,
) -> Result<Vec<&'a str>, String> {
    let values: Vec<_> = value.split(',').collect();
    if values.len() < min || values.len() > max || values.iter().any(|v| v.is_empty()) {
        Err(format!("line {line}: malformed {kind} record"))
    } else {
        Ok(values)
    }
}

pub fn number(value: &str, line: usize) -> Result<u32, String> {
    value
        .parse()
        .map_err(|_| format!("line {line}: invalid unsigned integer {value:?}"))
}

pub fn hits(value: &str, line: usize) -> Result<u64, String> {
    value
        .parse()
        .map_err(|_| format!("line {line}: invalid hit count {value:?}"))
}

pub fn branch_hits(value: &str, line: usize) -> Result<Option<u64>, String> {
    if value == "-" {
        Ok(None)
    } else {
        hits(value, line).map(Some)
    }
}

pub fn insert_hit<K: Ord>(
    map: &mut BTreeMap<K, u64>,
    key: K,
    value: u64,
    line: usize,
    kind: &str,
) -> Result<(), String> {
    if map.insert(key, value).is_some() {
        Err(format!("line {line}: duplicate {kind} key in one record"))
    } else {
        Ok(())
    }
}
