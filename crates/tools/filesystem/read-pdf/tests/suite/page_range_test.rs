use loopal_tool_read_pdf::parse_page_range;

#[test]
fn test_parse_single_page() {
    let result = parse_page_range("3", 10).unwrap();
    assert_eq!(result, vec![2]);
}

#[test]
fn test_parse_page_range_inclusive() {
    let result = parse_page_range("2-5", 10).unwrap();
    assert_eq!(result, vec![1, 2, 3, 4]);
}

#[test]
fn test_parse_range_clamped_to_total() {
    let result = parse_page_range("3-20", 5).unwrap();
    assert_eq!(result, vec![2, 3, 4]);
}

#[test]
fn test_parse_first_page() {
    let result = parse_page_range("1", 1).unwrap();
    assert_eq!(result, vec![0]);
}

#[test]
fn test_parse_page_zero_error() {
    let result = parse_page_range("0", 10);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("1-based"));
}

#[test]
fn test_parse_range_zero_start_error() {
    let result = parse_page_range("0-5", 10);
    assert!(result.is_err());
}

#[test]
fn test_parse_page_exceeds_total() {
    let result = parse_page_range("11", 10);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("exceeds total"));
}

#[test]
fn test_parse_range_start_exceeds_total() {
    let result = parse_page_range("11-15", 10);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("exceeds total"));
}

#[test]
fn test_parse_inverted_range_error() {
    let result = parse_page_range("5-3", 10);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("start"));
}

#[test]
fn test_parse_empty_spec_error() {
    let result = parse_page_range("", 10);
    assert!(result.is_err());
}

#[test]
fn test_parse_malformed_spec_error() {
    let result = parse_page_range("abc", 10);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("invalid page number"));
}
