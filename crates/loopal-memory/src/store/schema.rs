use rusqlite::Connection;

use loopal_error::MemoryGraphError;

const SCHEMA_SQL: &str = include_str!("schema.sql");

pub fn apply_schema(conn: &Connection) -> Result<(), MemoryGraphError> {
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    conn.execute_batch(SCHEMA_SQL)
        .map_err(|e| MemoryGraphError::SchemaInit(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open() -> Connection {
        Connection::open_in_memory().unwrap()
    }

    #[test]
    fn apply_creates_all_tables() {
        let conn = open();
        apply_schema(&conn).unwrap();

        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        assert!(tables.contains(&"memory_nodes".to_string()));
        assert!(tables.contains(&"memory_edges".to_string()));
        assert!(tables.contains(&"memory_files".to_string()));
        assert!(tables.contains(&"memory_unresolved_links".to_string()));
        assert!(tables.contains(&"schema_versions".to_string()));
    }

    #[test]
    fn apply_is_idempotent() {
        let conn = open();
        apply_schema(&conn).unwrap();
        apply_schema(&conn).unwrap();
        apply_schema(&conn).unwrap();
    }

    #[test]
    fn fts_virtual_table_exists() {
        let conn = open();
        apply_schema(&conn).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name='memory_fts'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn foreign_keys_enabled() {
        let conn = open();
        apply_schema(&conn).unwrap();
        let on: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .unwrap();
        assert_eq!(on, 1);
    }
}
