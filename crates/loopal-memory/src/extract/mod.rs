use chrono::Utc;
use sha2::{Digest, Sha256};

pub mod errors;
pub mod frontmatter;
pub mod related;
pub mod wikilink;

use crate::extract::errors::ExtractionError;
use crate::extract::frontmatter::{ParsedFile, parse};
use crate::extract::related::normalize_related;
use crate::extract::wikilink::scan;
use crate::store::types::{EdgeKind, MemoryEdge, MemoryKind, MemoryNode, Provenance};

pub struct ExtractionResult {
    pub nodes: Vec<MemoryNode>,
    pub edges: Vec<MemoryEdge>,
    pub unresolved: Vec<UnresolvedLink>,
    pub errors: Vec<ExtractionError>,
}

pub struct UnresolvedLink {
    pub from_id: String,
    pub target_name: String,
    pub line: u32,
}

pub fn extract_file(file_path: &str, content: &str) -> ExtractionResult {
    let now = Utc::now().timestamp_millis();
    let parsed = parse(content);
    let slug = slug_from_path(file_path);
    let kind = pick_kind(&parsed.frontmatter.kind, &slug);
    let name = parsed
        .frontmatter
        .name
        .clone()
        .unwrap_or_else(|| slug.clone());
    let content_hash = sha256_hex(content);
    // reason: store the whole note body so full-text search reaches content past
    // the first few hundred bytes; display sites truncate to a preview.
    let body = parsed.body.trim_start().to_string();
    let created_at = parse_iso_date(parsed.frontmatter.created_at.as_deref()).unwrap_or(now);
    let updated_at = parse_iso_date(parsed.frontmatter.updated_at.as_deref()).unwrap_or(now);

    let node = MemoryNode {
        id: slug.clone(),
        kind,
        name,
        description: parsed.frontmatter.description.clone(),
        file_path: file_path.to_string(),
        body,
        created_at,
        updated_at,
        ttl_days: parsed.frontmatter.ttl_days,
        content_hash,
        indexed_at: now,
    };

    let (edges, unresolved) = build_edges(&slug, &parsed, now);

    ExtractionResult {
        nodes: vec![node],
        edges,
        unresolved,
        errors: parsed.errors,
    }
}

fn parse_iso_date(s: Option<&str>) -> Option<i64> {
    let s = s?.trim();
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp_millis());
    }
    let date = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()?;
    let dt = date.and_hms_opt(0, 0, 0)?.and_utc();
    Some(dt.timestamp_millis())
}

fn build_edges(
    slug: &str,
    parsed: &ParsedFile,
    now: i64,
) -> (Vec<MemoryEdge>, Vec<UnresolvedLink>) {
    let mut edges = Vec::new();
    let mut unresolved = Vec::new();

    for target in normalize_related(&parsed.frontmatter.related) {
        edges.push(MemoryEdge {
            id: None,
            src_id: slug.to_string(),
            dst_id: target,
            kind: EdgeKind::References,
            line: None,
            metadata: None,
            provenance: Provenance::Frontmatter,
            confidence: 1.0,
            created_at: now,
        });
    }

    for link in scan(&parsed.body) {
        edges.push(MemoryEdge {
            id: None,
            src_id: slug.to_string(),
            dst_id: link.slug.clone(),
            kind: EdgeKind::References,
            line: Some(link.line),
            metadata: None,
            provenance: Provenance::InlineLink,
            confidence: 1.0,
            created_at: now,
        });
        unresolved.push(UnresolvedLink {
            from_id: slug.to_string(),
            target_name: link.slug,
            line: link.line,
        });
    }

    (edges, unresolved)
}

pub fn slug_from_path(file_path: &str) -> String {
    let trimmed = file_path.strip_prefix("./").unwrap_or(file_path);
    let without_ext = trimmed.strip_suffix(".md").unwrap_or(trimmed);
    if without_ext.is_empty() {
        return file_path.to_string();
    }
    without_ext.replace([std::path::MAIN_SEPARATOR, '/'], "__")
}

fn pick_kind(declared: &Option<String>, slug: &str) -> MemoryKind {
    if slug.eq_ignore_ascii_case("MEMORY") {
        return MemoryKind::Index;
    }
    match declared.as_deref() {
        Some("user") => MemoryKind::User,
        Some("feedback") => MemoryKind::Feedback,
        Some("project") => MemoryKind::Project,
        Some("reference") => MemoryKind::Reference,
        Some("index") => MemoryKind::Index,
        _ => MemoryKind::Reference,
    }
}

fn sha256_hex(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    format!("{:x}", hasher.finalize())
}
