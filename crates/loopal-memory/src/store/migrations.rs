use rusqlite::{Connection, params};

use loopal_error::MemoryGraphError;

pub const CURRENT_SCHEMA_VERSION: u32 = 2;

/// The per-session graph DB is a rebuildable derived index over the `.md` source
/// of truth. If it was created by an older schema, drop the derived tables so
/// `apply_schema` recreates them with the current shape; the next directory scan
/// repopulates them from disk.
pub fn reset_if_outdated(conn: &Connection) -> Result<(), MemoryGraphError> {
    let has_versions: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='schema_versions'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);
    if !has_versions {
        return Ok(());
    }
    let recorded: Option<i64> = conn
        .query_row("SELECT max(version) FROM schema_versions", [], |r| {
            r.get::<_, Option<i64>>(0)
        })
        .ok()
        .flatten();
    let stale = matches!(recorded, Some(v) if (v as u32) < CURRENT_SCHEMA_VERSION);
    if !stale {
        return Ok(());
    }
    conn.execute_batch(
        "DROP TABLE IF EXISTS memory_fts;
         DROP TABLE IF EXISTS memory_unresolved_links;
         DROP TABLE IF EXISTS memory_edges;
         DROP TABLE IF EXISTS memory_nodes;
         DROP TABLE IF EXISTS memory_files;
         DELETE FROM schema_versions;",
    )?;
    Ok(())
}

pub fn ensure_version(conn: &Connection, version: u32) -> Result<(), MemoryGraphError> {
    let exists: Option<i64> = conn
        .query_row(
            "SELECT version FROM schema_versions WHERE version = ?1",
            params![version],
            |row| row.get(0),
        )
        .ok();

    if exists.is_none() {
        let now_ms = chrono::Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO schema_versions (version, applied_at, description) VALUES (?1, ?2, ?3)",
            params![version, now_ms, format!("schema v{}", version)],
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::schema::apply_schema;

    fn open() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        apply_schema(&conn).unwrap();
        conn
    }

    #[test]
    fn ensure_version_inserts_once() {
        let conn = open();
        ensure_version(&conn, 1).unwrap();
        ensure_version(&conn, 1).unwrap();
        let count: i64 = conn
            .query_row("SELECT count(*) FROM schema_versions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn reset_wipes_derived_tables_on_older_schema() {
        let conn = Connection::open_in_memory().unwrap();
        apply_schema(&conn).unwrap();
        ensure_version(&conn, 1).unwrap();
        conn.execute(
            "INSERT INTO memory_nodes (id, kind, name, file_path, body, created_at, updated_at, content_hash, indexed_at)
             VALUES ('x','project','n','x.md','b',1,1,'h',1)",
            [],
        )
        .unwrap();
        reset_if_outdated(&conn).unwrap();
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='memory_nodes'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);
        assert!(
            !exists,
            "outdated derived tables should be dropped for rebuild"
        );
    }

    #[test]
    fn reset_keeps_tables_on_current_schema() {
        let conn = Connection::open_in_memory().unwrap();
        apply_schema(&conn).unwrap();
        ensure_version(&conn, CURRENT_SCHEMA_VERSION).unwrap();
        reset_if_outdated(&conn).unwrap();
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='memory_nodes'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);
        assert!(exists, "current-schema tables must be preserved");
    }
}
