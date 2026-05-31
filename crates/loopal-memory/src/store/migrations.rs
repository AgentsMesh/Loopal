use rusqlite::{Connection, params};

use loopal_error::MemoryGraphError;

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

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
}
