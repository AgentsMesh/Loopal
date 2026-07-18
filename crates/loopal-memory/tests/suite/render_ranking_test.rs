use loopal_memory::render::ranking::rank;
use loopal_memory::{MemoryKind, MemoryNode};

fn node(id: &str, kind: MemoryKind) -> MemoryNode {
    MemoryNode {
        id: id.into(),
        kind,
        name: id.into(),
        description: None,
        file_path: format!("{}.md", id),
        body: id.into(),
        created_at: chrono::Utc::now().timestamp_millis(),
        updated_at: chrono::Utc::now().timestamp_millis(),
        ttl_days: None,
        content_hash: "h".repeat(64),
        indexed_at: 1,
    }
}

#[test]
fn higher_incoming_count_yields_higher_score() {
    let a = rank(node("a", MemoryKind::Project), 0);
    let b = rank(node("b", MemoryKind::Project), 5);
    assert!(b.score > a.score);
}

#[test]
fn project_outranks_reference_at_same_metrics() {
    let p = rank(node("p", MemoryKind::Project), 1);
    let r = rank(node("r", MemoryKind::Reference), 1);
    assert!(p.score > r.score);
}

#[test]
fn orphan_node_gets_penalty() {
    let with_inbound = rank(node("a", MemoryKind::Project), 1);
    let orphan = rank(node("b", MemoryKind::Project), 0);
    assert!(with_inbound.score > orphan.score);
}

#[test]
fn expired_ttl_drags_score_down() {
    let now = chrono::Utc::now().timestamp_millis();
    let day_ms: i64 = 1000 * 60 * 60 * 24;
    let mut fresh = node("f", MemoryKind::Project);
    fresh.created_at = now;
    fresh.updated_at = now;
    fresh.ttl_days = Some(90);

    let mut expired = node("e", MemoryKind::Project);
    expired.created_at = now - 200 * day_ms;
    expired.updated_at = now - 200 * day_ms;
    expired.ttl_days = Some(90);

    let fs = rank(fresh, 1);
    let es = rank(expired, 1);
    assert!(fs.score > es.score);
}

#[test]
fn index_type_weight_is_zero() {
    let i = rank(node("idx", MemoryKind::Index), 0);
    let p = rank(node("p", MemoryKind::Project), 0);
    assert!(p.score > i.score);
}
