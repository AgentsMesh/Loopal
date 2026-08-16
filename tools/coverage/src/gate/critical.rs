use std::collections::{BTreeMap, BTreeSet};

pub(super) fn hits(
    functions: &BTreeMap<String, u64>,
    lines: &BTreeMap<String, u32>,
    name: &str,
) -> Option<u64> {
    let exact: Vec<_> = functions
        .keys()
        .filter(|symbol| symbol_is_exact(symbol, name))
        .collect();
    let matches: Vec<_> = if exact.is_empty() {
        functions
            .keys()
            .filter(|symbol| symbol_is_generic(symbol, name))
            .collect()
    } else {
        exact
    };
    let candidate_lines: BTreeSet<_> = matches
        .iter()
        .filter_map(|symbol| lines.get(*symbol))
        .collect();
    if candidate_lines.len() != 1 {
        return None;
    }
    Some(matches.into_iter().fold(0u64, |total, symbol| {
        total.saturating_add(functions[symbol])
    }))
}

fn symbol_is_exact(symbol: &str, name: &str) -> bool {
    symbol == name
        || symbol.ends_with(&format!("::{name}"))
        || (symbol.starts_with("_R") && symbol.ends_with(&format!("{}{name}", name.len())))
}

fn symbol_is_generic(symbol: &str, name: &str) -> bool {
    if !symbol.starts_with("_R") || symbol.starts_with("_RNC") {
        return false;
    }
    let encoded = format!("{}{name}", name.len());
    symbol.match_indices(&encoded).any(|(index, _)| {
        symbol[index + encoded.len()..]
            .chars()
            .next()
            .is_none_or(|next| !next.is_ascii_digit())
    })
}
