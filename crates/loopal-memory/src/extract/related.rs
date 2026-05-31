use crate::extract::wikilink::normalize_to_slug;

pub fn normalize_related(items: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(items.len());
    let mut seen = std::collections::HashSet::new();
    for item in items {
        if let Some(slug) = normalize_to_slug(item)
            && seen.insert(slug.clone())
        {
            out.push(slug);
        }
    }
    out
}
