pub(crate) mod read;
pub(crate) mod write;

pub(crate) use read::*;
pub(crate) use write::*;

use rusqlite::Row;

use crate::store::types::{MemoryKind, MemoryNode};

pub(super) fn map_row(row: &Row<'_>) -> rusqlite::Result<MemoryNode> {
    let kind_str: String = row.get("kind")?;
    let kind = MemoryKind::parse(&kind_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
    })?;
    Ok(MemoryNode {
        id: row.get("id")?,
        kind,
        name: row.get("name")?,
        description: row.get("description")?,
        file_path: row.get("file_path")?,
        body_preview: row.get("body_preview")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        ttl_days: row.get("ttl_days")?,
        content_hash: row.get("content_hash")?,
        indexed_at: row.get("indexed_at")?,
    })
}
