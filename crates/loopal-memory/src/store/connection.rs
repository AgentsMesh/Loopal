use std::path::Path;
use std::sync::Arc;

use rusqlite::Connection;
use tokio::sync::Mutex;

use loopal_error::MemoryGraphError;

use crate::store::migrations::{CURRENT_SCHEMA_VERSION, ensure_version};
use crate::store::schema::apply_schema;

#[derive(Clone)]
pub struct DbConnection {
    inner: Arc<Mutex<Connection>>,
}

impl DbConnection {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, MemoryGraphError> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path.as_ref())?;
        Self::init(conn)
    }

    pub fn in_memory() -> Result<Self, MemoryGraphError> {
        let conn = Connection::open_in_memory()?;
        Self::init(conn)
    }

    fn init(conn: Connection) -> Result<Self, MemoryGraphError> {
        apply_schema(&conn)?;
        ensure_version(&conn, CURRENT_SCHEMA_VERSION)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(conn)),
        })
    }

    pub async fn with_conn<F, R>(&self, f: F) -> Result<R, MemoryGraphError>
    where
        F: FnOnce(&Connection) -> Result<R, MemoryGraphError>,
    {
        let guard = self.inner.lock().await;
        f(&guard)
    }

    pub async fn with_conn_mut<F, R>(&self, f: F) -> Result<R, MemoryGraphError>
    where
        F: FnOnce(&mut Connection) -> Result<R, MemoryGraphError>,
    {
        let mut guard = self.inner.lock().await;
        f(&mut guard)
    }
}
