-- DiskMate local index, initial schema.
--
-- Design notes that matter for anyone changing this file:
--
--  * `fs_entries.path` is populated for DIRECTORIES ONLY. A file's path is
--    `parent.path || '/' || name`, resolved with a single join at query time.
--    Storing it per file would roughly triple the index size on a real volume.
--  * Subtree totals live on the directory row. They are computed in Rust while
--    walking and written in one `UPDATE ... FROM` pass at the end of a scan, so
--    the UI never has to sum anything.
--  * Query indexes are created by the writer AFTER bulk load (see
--    `db::writer`), not here, so that the load path only maintains the rowid
--    primary key. This file creates just the structural constraints.

CREATE TABLE volumes (
    id                INTEGER PRIMARY KEY,
    volume_key        TEXT    NOT NULL UNIQUE,
    name              TEXT    NOT NULL,
    mount_point       TEXT    NOT NULL,
    device            TEXT,
    fs_type           TEXT    NOT NULL,
    kind              TEXT    NOT NULL,
    total_bytes       INTEGER,
    available_bytes   INTEGER,
    is_removable      INTEGER NOT NULL DEFAULT 0,
    is_read_only      INTEGER NOT NULL DEFAULT 0,
    first_seen_at     INTEGER NOT NULL,
    last_seen_at      INTEGER NOT NULL
);

CREATE TABLE scans (
    id                  INTEGER PRIMARY KEY,
    volume_id           INTEGER REFERENCES volumes(id) ON DELETE SET NULL,
    root_path           TEXT    NOT NULL,
    status              TEXT    NOT NULL,
    started_at          INTEGER NOT NULL,
    finished_at         INTEGER,
    duration_ms         INTEGER NOT NULL DEFAULT 0,
    file_count          INTEGER NOT NULL DEFAULT 0,
    dir_count           INTEGER NOT NULL DEFAULT 0,
    symlink_count       INTEGER NOT NULL DEFAULT 0,
    other_count         INTEGER NOT NULL DEFAULT 0,
    error_count         INTEGER NOT NULL DEFAULT 0,
    logical_bytes       INTEGER NOT NULL DEFAULT 0,
    allocated_bytes     INTEGER NOT NULL DEFAULT 0,
    entries_per_second  REAL    NOT NULL DEFAULT 0,
    root_entry_id       INTEGER,
    options_json        TEXT    NOT NULL DEFAULT '{}',
    engine_version      TEXT    NOT NULL DEFAULT ''
);

CREATE INDEX idx_scans_volume ON scans(volume_id, started_at DESC);

-- One row per filesystem entry. Deliberately narrow: every column here is read
-- by a query the product actually makes.
CREATE TABLE fs_entries (
    id                      INTEGER PRIMARY KEY,
    scan_id                 INTEGER NOT NULL,
    parent_id               INTEGER,
    name                    TEXT    NOT NULL,
    path                    TEXT,              -- directories only
    depth                   INTEGER NOT NULL,
    kind                    INTEGER NOT NULL,  -- 0 file 1 dir 2 symlink 3 other
    logical_size            INTEGER NOT NULL DEFAULT 0,
    allocated_size          INTEGER NOT NULL DEFAULT 0,
    subtree_logical_size    INTEGER NOT NULL DEFAULT 0,
    subtree_allocated_size  INTEGER NOT NULL DEFAULT 0,
    subtree_file_count      INTEGER NOT NULL DEFAULT 0,
    subtree_dir_count       INTEGER NOT NULL DEFAULT 0,
    extension               TEXT,
    modified_at             INTEGER,
    created_at              INTEGER,
    accessed_at             INTEGER,
    mode                    INTEGER NOT NULL DEFAULT 0,
    uid                     INTEGER NOT NULL DEFAULT 0,
    gid                     INTEGER NOT NULL DEFAULT 0,
    inode                   INTEGER NOT NULL DEFAULT 0,
    device                  INTEGER NOT NULL DEFAULT 0,
    nlink                   INTEGER NOT NULL DEFAULT 1,
    is_hidden               INTEGER NOT NULL DEFAULT 0,
    hardlink_dup            INTEGER NOT NULL DEFAULT 0,
    accessible              INTEGER NOT NULL DEFAULT 1,
    category                INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE scan_errors (
    id           INTEGER PRIMARY KEY,
    scan_id      INTEGER NOT NULL REFERENCES scans(id) ON DELETE CASCADE,
    path         TEXT    NOT NULL,
    kind         TEXT    NOT NULL,
    code         INTEGER,
    message      TEXT    NOT NULL,
    occurred_at  INTEGER NOT NULL
);

CREATE INDEX idx_scan_errors_scan ON scan_errors(scan_id, kind);

-- Benchmark ledger. Written by the bench harness and by instrumented scans so
-- scanner revisions can be compared objectively over time.
CREATE TABLE scan_benchmarks (
    id                 INTEGER PRIMARY KEY,
    scan_id            INTEGER REFERENCES scans(id) ON DELETE CASCADE,
    label              TEXT    NOT NULL,
    engine_version     TEXT    NOT NULL,
    host_os            TEXT    NOT NULL,
    host_arch          TEXT    NOT NULL,
    threads            INTEGER NOT NULL,
    batch_size         INTEGER NOT NULL,
    duration_ms        INTEGER NOT NULL,
    entries            INTEGER NOT NULL,
    entries_per_second REAL    NOT NULL,
    bytes_indexed      INTEGER NOT NULL,
    index_bytes        INTEGER NOT NULL,
    peak_rss_bytes     INTEGER,
    errors             INTEGER NOT NULL DEFAULT 0,
    recorded_at        INTEGER NOT NULL
);

-- Pre-aggregated breakdowns.
--
-- Category and extension totals are accumulated in Rust while walking (a few
-- thousand distinct extensions at most) and written once. That removes the need
-- for an index on `fs_entries.extension`, which would have cost more in write
-- throughput on a multi-million-file volume than the breakdown query is worth.
CREATE TABLE scan_categories (
    scan_id     INTEGER NOT NULL,
    category    INTEGER NOT NULL,
    file_count  INTEGER NOT NULL,
    total_bytes INTEGER NOT NULL,
    PRIMARY KEY (scan_id, category)
) WITHOUT ROWID;

CREATE TABLE scan_extensions (
    scan_id     INTEGER NOT NULL,
    extension   TEXT    NOT NULL,  -- '' means the file had no extension
    file_count  INTEGER NOT NULL,
    total_bytes INTEGER NOT NULL,
    PRIMARY KEY (scan_id, extension)
) WITHOUT ROWID;
