/// Result of a search-and-replace operation.
pub enum SearchReplaceResult {
    /// Successfully replaced. Contains the new file content.
    Ok(String),
    /// The search string was not found.
    NotFound,
    /// The search string was found multiple times (when replace_all is false).
    MultipleMatches(usize),
}

/// Perform exact string search and replace on `content`.
///
/// If `replace_all` is false, the `old_string` must appear exactly once.
/// If `replace_all` is true, all occurrences are replaced.
pub fn search_replace(
    content: &str,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
) -> SearchReplaceResult {
    let count = content.matches(old_string).count();

    if count == 0 {
        return SearchReplaceResult::NotFound;
    }

    if !replace_all && count > 1 {
        return SearchReplaceResult::MultipleMatches(count);
    }

    if replace_all {
        SearchReplaceResult::Ok(content.replace(old_string, new_string))
    } else {
        SearchReplaceResult::Ok(content.replacen(old_string, new_string, 1))
    }
}