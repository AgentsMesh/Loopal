use crate::fixture::Fixture;

const BYTES_PER_TOKEN: usize = 4;

pub fn read_all_baseline(fixtures: &[Fixture]) -> usize {
    let total_bytes: usize = fixtures.iter().map(|f| f.bytes()).sum();
    total_bytes / BYTES_PER_TOKEN
}

pub fn grep_filter_baseline(fixtures: &[Fixture], query: &str) -> usize {
    let needles: Vec<String> = query
        .split_whitespace()
        .map(|s| s.to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    if needles.is_empty() {
        return read_all_baseline(fixtures);
    }
    let bytes: usize = fixtures
        .iter()
        .filter(|f| {
            let lower = f.content.to_lowercase();
            needles.iter().any(|n| lower.contains(n))
        })
        .map(|f| f.bytes())
        .sum();
    bytes / BYTES_PER_TOKEN
}

pub fn recall_output_tokens(formatted: &str) -> usize {
    formatted.len() / BYTES_PER_TOKEN
}

pub fn savings_percent(baseline: usize, current: usize) -> f32 {
    if baseline == 0 {
        return 0.0;
    }
    let saved = baseline.saturating_sub(current) as f32;
    (saved / baseline as f32) * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fix(_slug: &'static str, content: &'static str) -> Fixture {
        Fixture {
            file_path: "p",
            content,
        }
    }

    #[test]
    fn read_all_sums_all_bytes() {
        let fixtures = vec![fix("a", "12345678"), fix("b", "abcd")];
        assert_eq!(read_all_baseline(&fixtures), 3);
    }

    #[test]
    fn grep_filter_returns_full_when_no_query() {
        let fixtures = vec![fix("a", "1234")];
        assert_eq!(grep_filter_baseline(&fixtures, ""), 1);
    }

    #[test]
    fn grep_filter_only_counts_matching() {
        let fixtures = vec![fix("a", "twitter content here"), fix("b", "scanner stuff")];
        let tokens = grep_filter_baseline(&fixtures, "twitter");
        assert_eq!(tokens, 20 / BYTES_PER_TOKEN);
    }

    #[test]
    fn recall_tokens_divides_by_four() {
        assert_eq!(recall_output_tokens("12345678"), 2);
    }

    #[test]
    fn savings_clamps_at_zero_when_current_exceeds_baseline() {
        assert_eq!(savings_percent(100, 200), 0.0);
        assert!((savings_percent(100, 30) - 70.0).abs() < 1e-3);
    }
}
