use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use unicode_segmentation::UnicodeSegmentation;

static STOPWORDS_EN: &[&str] = &[
    "the", "a", "an", "and", "or", "but", "if", "then", "else", "of", "to", "in", "on", "at", "by",
    "for", "with", "from", "this", "that", "these", "those", "is", "are", "was", "were", "be",
    "been", "being", "have", "has", "had", "do", "does", "did", "will", "would", "could", "should",
    "may", "might", "must", "can", "not", "no", "nor", "so", "as", "it", "its", "they", "them",
    "their", "you", "your", "we", "our", "us", "i", "me", "my", "over", "under", "into", "onto",
    "out", "up", "down", "off",
];

static STOPWORDS_ZH: &[&str] = &[
    "的", "了", "和", "是", "在", "我", "有", "不", "也", "都", "就", "要", "这", "那", "你", "他",
    "她", "它", "们", "上", "下", "中", "对", "与", "为", "等", "及", "或", "但", "而", "如", "于",
    "被", "由", "把", "从", "到", "之",
];

static STOPWORD_SET: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    STOPWORDS_EN
        .iter()
        .chain(STOPWORDS_ZH.iter())
        .copied()
        .collect()
});

fn meets_min_len(token: &str) -> bool {
    let chars = token.chars().count();
    if chars == 0 {
        return false;
    }
    if token.is_ascii() { chars >= 2 } else { true }
}

pub fn tokenize(text: &str) -> Vec<String> {
    text.unicode_words()
        .map(|w| w.to_lowercase())
        .filter(|w| meets_min_len(w))
        .filter(|w| !STOPWORD_SET.contains(w.as_str()))
        .collect()
}

pub fn term_frequency(tokens: &[String]) -> HashMap<String, f32> {
    let mut tf: HashMap<String, f32> = HashMap::new();
    for t in tokens {
        *tf.entry(t.clone()).or_insert(0.0) += 1.0;
    }
    let n = tokens.len() as f32;
    if n > 0.0 {
        for v in tf.values_mut() {
            *v /= n;
        }
    }
    tf
}

pub fn document_frequency(per_doc_tokens: &[Vec<String>]) -> HashMap<String, usize> {
    let mut df: HashMap<String, usize> = HashMap::new();
    for tokens in per_doc_tokens {
        let unique: HashSet<&String> = tokens.iter().collect();
        for t in unique {
            *df.entry(t.clone()).or_insert(0) += 1;
        }
    }
    df
}

pub fn tf_idf_vector(
    tf: &HashMap<String, f32>,
    df: &HashMap<String, usize>,
    n_docs: usize,
) -> HashMap<String, f32> {
    let mut out = HashMap::with_capacity(tf.len());
    let n = n_docs as f32;
    for (term, &freq) in tf {
        let doc_freq = *df.get(term).unwrap_or(&1) as f32;
        let idf = ((n + 1.0) / (doc_freq + 1.0)).ln();
        out.insert(term.clone(), freq * idf);
    }
    out
}

pub fn cosine_similarity(a: &HashMap<String, f32>, b: &HashMap<String, f32>) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0_f32;
    for (term, va) in a {
        if let Some(vb) = b.get(term) {
            dot += va * vb;
        }
    }
    let norm_a: f32 = a.values().map(|v| v * v).sum::<f32>().sqrt();
    let norm_b: f32 = b.values().map(|v| v * v).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}
