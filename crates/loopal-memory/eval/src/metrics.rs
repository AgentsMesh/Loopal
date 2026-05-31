use std::collections::{HashMap, HashSet};

pub fn precision_at_k(relevant: &HashSet<String>, retrieved: &[String], k: usize) -> f32 {
    if k == 0 {
        return 0.0;
    }
    let window = &retrieved[..retrieved.len().min(k)];
    if window.is_empty() {
        return 0.0;
    }
    let hits = window.iter().filter(|id| relevant.contains(*id)).count();
    hits as f32 / window.len() as f32
}

pub fn recall_at_k(relevant: &HashSet<String>, retrieved: &[String], k: usize) -> f32 {
    if relevant.is_empty() {
        return 0.0;
    }
    let window = &retrieved[..retrieved.len().min(k)];
    let hits = window.iter().filter(|id| relevant.contains(*id)).count();
    hits as f32 / relevant.len() as f32
}

pub fn f1_at_k(relevant: &HashSet<String>, retrieved: &[String], k: usize) -> f32 {
    let p = precision_at_k(relevant, retrieved, k);
    let r = recall_at_k(relevant, retrieved, k);
    if p + r == 0.0 {
        0.0
    } else {
        2.0 * p * r / (p + r)
    }
}

pub fn mrr(relevant: &HashSet<String>, retrieved: &[String]) -> f32 {
    for (idx, id) in retrieved.iter().enumerate() {
        if relevant.contains(id) {
            return 1.0 / (idx + 1) as f32;
        }
    }
    0.0
}

pub fn ndcg_at_k(relevance_map: &HashMap<String, u32>, retrieved: &[String], k: usize) -> f32 {
    let dcg = dcg_at_k(relevance_map, retrieved, k);
    let mut sorted_grades: Vec<u32> = relevance_map.values().copied().collect();
    sorted_grades.sort_by(|a, b| b.cmp(a));
    let ideal_ids: Vec<String> = (0..sorted_grades.len())
        .map(|i| format!("__ideal_{i}"))
        .collect();
    let mut ideal_map: HashMap<String, u32> = HashMap::new();
    for (id, grade) in ideal_ids.iter().zip(sorted_grades.iter()) {
        ideal_map.insert(id.clone(), *grade);
    }
    let idcg = dcg_at_k(&ideal_map, &ideal_ids, k);
    if idcg == 0.0 { 0.0 } else { dcg / idcg }
}

fn dcg_at_k(relevance_map: &HashMap<String, u32>, retrieved: &[String], k: usize) -> f32 {
    let mut sum = 0.0_f32;
    for (idx, id) in retrieved.iter().take(k).enumerate() {
        let rel = *relevance_map.get(id).unwrap_or(&0) as f32;
        // reason: rank index is 0-based here; log2(i+2) matches the standard DCG formula
        let denom = ((idx + 2) as f32).log2();
        sum += rel / denom;
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    fn relevant_set(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    fn retrieved_vec(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn precision_perfect_hit() {
        let rel = relevant_set(&["a", "b"]);
        let ret = retrieved_vec(&["a", "b", "c"]);
        assert_eq!(precision_at_k(&rel, &ret, 2), 1.0);
    }

    #[test]
    fn precision_zero_when_no_overlap() {
        let rel = relevant_set(&["a", "b"]);
        let ret = retrieved_vec(&["x", "y"]);
        assert_eq!(precision_at_k(&rel, &ret, 2), 0.0);
    }

    #[test]
    fn recall_caps_at_one() {
        let rel = relevant_set(&["a", "b"]);
        let ret = retrieved_vec(&["a", "b", "c", "d"]);
        assert_eq!(recall_at_k(&rel, &ret, 4), 1.0);
    }

    #[test]
    fn f1_zero_when_both_zero() {
        let rel = relevant_set(&["a"]);
        let ret = retrieved_vec(&["x"]);
        assert_eq!(f1_at_k(&rel, &ret, 1), 0.0);
    }

    #[test]
    fn mrr_uses_first_hit_rank() {
        let rel = relevant_set(&["b"]);
        let ret = retrieved_vec(&["a", "b", "c"]);
        assert!((mrr(&rel, &ret) - 0.5).abs() < 1e-5);
    }

    #[test]
    fn mrr_zero_when_no_hit() {
        let rel = relevant_set(&["z"]);
        let ret = retrieved_vec(&["a", "b"]);
        assert_eq!(mrr(&rel, &ret), 0.0);
    }

    #[test]
    fn ndcg_perfect_ordering_yields_one() {
        let mut map = HashMap::new();
        map.insert("a".into(), 3);
        map.insert("b".into(), 2);
        map.insert("c".into(), 1);
        let ret = retrieved_vec(&["a", "b", "c"]);
        let score = ndcg_at_k(&map, &ret, 3);
        assert!(score > 0.99);
    }

    #[test]
    fn ndcg_reversed_ordering_below_one() {
        let mut map = HashMap::new();
        map.insert("a".into(), 3);
        map.insert("b".into(), 1);
        let ret = retrieved_vec(&["b", "a"]);
        let score = ndcg_at_k(&map, &ret, 2);
        assert!(score < 1.0 && score > 0.5);
    }

    #[test]
    fn handles_k_larger_than_retrieved() {
        let rel = relevant_set(&["a", "b"]);
        let ret = retrieved_vec(&["a"]);
        assert_eq!(precision_at_k(&rel, &ret, 10), 1.0);
        assert_eq!(recall_at_k(&rel, &ret, 10), 0.5);
    }
}
