CREATE TABLE IF NOT EXISTS schema_versions (
    version     INTEGER PRIMARY KEY,
    applied_at  INTEGER NOT NULL,
    description TEXT
);

CREATE TABLE IF NOT EXISTS memory_nodes (
    id            TEXT PRIMARY KEY,
    kind          TEXT NOT NULL,
    name          TEXT NOT NULL,
    description   TEXT,
    file_path     TEXT NOT NULL,
    body_preview  TEXT,
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL,
    ttl_days      INTEGER,
    content_hash  TEXT NOT NULL,
    indexed_at    INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_memory_nodes_kind      ON memory_nodes(kind);
CREATE UNIQUE INDEX IF NOT EXISTS uniq_memory_nodes_file_path ON memory_nodes(file_path);
CREATE INDEX IF NOT EXISTS idx_memory_nodes_name      ON memory_nodes(lower(name));

CREATE TABLE IF NOT EXISTS memory_edges (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    src_id      TEXT NOT NULL,
    dst_id      TEXT NOT NULL,
    kind        TEXT NOT NULL,
    line        INTEGER,
    metadata    TEXT,
    provenance  TEXT NOT NULL,
    confidence  REAL NOT NULL DEFAULT 1.0,
    created_at  INTEGER NOT NULL,
    FOREIGN KEY (src_id) REFERENCES memory_nodes(id) ON DELETE CASCADE,
    FOREIGN KEY (dst_id) REFERENCES memory_nodes(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_memory_edges_kind         ON memory_edges(kind);
CREATE INDEX IF NOT EXISTS idx_memory_edges_src_kind     ON memory_edges(src_id, kind);
CREATE INDEX IF NOT EXISTS idx_memory_edges_dst_kind     ON memory_edges(dst_id, kind);
CREATE INDEX IF NOT EXISTS idx_memory_edges_provenance   ON memory_edges(provenance);
CREATE UNIQUE INDEX IF NOT EXISTS uniq_memory_edges_triple
    ON memory_edges(src_id, dst_id, kind, provenance);

CREATE TABLE IF NOT EXISTS memory_files (
    path          TEXT PRIMARY KEY,
    content_hash  TEXT NOT NULL,
    size          INTEGER NOT NULL,
    modified_at   INTEGER NOT NULL,
    indexed_at    INTEGER NOT NULL,
    errors        TEXT
);

CREATE TABLE IF NOT EXISTS memory_unresolved_links (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    from_id     TEXT NOT NULL,
    target_name TEXT NOT NULL,
    line        INTEGER,
    FOREIGN KEY (from_id) REFERENCES memory_nodes(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_unresolved_from   ON memory_unresolved_links(from_id);
CREATE INDEX IF NOT EXISTS idx_unresolved_target ON memory_unresolved_links(target_name);

CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(
    id, name, description, body_preview,
    content='memory_nodes',
    content_rowid='rowid'
);

CREATE TRIGGER IF NOT EXISTS memory_nodes_ai AFTER INSERT ON memory_nodes BEGIN
    INSERT INTO memory_fts(rowid, id, name, description, body_preview)
    VALUES (NEW.rowid, NEW.id, NEW.name, NEW.description, NEW.body_preview);
END;

CREATE TRIGGER IF NOT EXISTS memory_nodes_ad AFTER DELETE ON memory_nodes BEGIN
    INSERT INTO memory_fts(memory_fts, rowid, id, name, description, body_preview)
    VALUES ('delete', OLD.rowid, OLD.id, OLD.name, OLD.description, OLD.body_preview);
END;

CREATE TRIGGER IF NOT EXISTS memory_nodes_au AFTER UPDATE ON memory_nodes BEGIN
    INSERT INTO memory_fts(memory_fts, rowid, id, name, description, body_preview)
    VALUES ('delete', OLD.rowid, OLD.id, OLD.name, OLD.description, OLD.body_preview);
    INSERT INTO memory_fts(rowid, id, name, description, body_preview)
    VALUES (NEW.rowid, NEW.id, NEW.name, NEW.description, NEW.body_preview);
END;
