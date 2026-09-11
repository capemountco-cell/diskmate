-- Extension points for analysis features that build on a completed scan.
--
-- These tables are created now and stay empty through Milestone 01. They exist
-- so that duplicate detection, classification, cleanup and snapshots can be
-- added later without restructuring `fs_entries` or rewriting the scanner.

-- Rule-driven classification, recomputable without rescanning. Kept separate
-- from `fs_entries.category` (the scanner's cheap inline guess) so that a
-- better ruleset can be applied to an existing index.
CREATE TABLE file_classifications (
    entry_id    INTEGER NOT NULL,
    scan_id     INTEGER NOT NULL,
    category    INTEGER NOT NULL,
    rule        TEXT    NOT NULL,
    confidence  REAL    NOT NULL DEFAULT 1.0,
    PRIMARY KEY (entry_id, rule)
) WITHOUT ROWID;

-- Content hashes for duplicate detection. `kind` distinguishes the cheap
-- partial probe from the full digest, so the escalating pipeline described in
-- docs/database.md can record both without a second table.
CREATE TABLE content_hashes (
    entry_id  INTEGER NOT NULL,
    scan_id   INTEGER NOT NULL,
    kind      TEXT    NOT NULL,   -- 'partial' | 'full'
    algorithm TEXT    NOT NULL,
    offset    INTEGER NOT NULL DEFAULT 0,
    length    INTEGER NOT NULL DEFAULT 0,
    digest    BLOB    NOT NULL,
    PRIMARY KEY (entry_id, kind)
) WITHOUT ROWID;

CREATE INDEX idx_content_hashes_lookup ON content_hashes(scan_id, kind, digest);

CREATE TABLE duplicate_groups (
    id             INTEGER PRIMARY KEY,
    scan_id        INTEGER NOT NULL,
    logical_size   INTEGER NOT NULL,
    member_count   INTEGER NOT NULL,
    -- Bytes recoverable if all but one copy were removed. Never presented as
    -- "safe to delete" on its own.
    reclaimable    INTEGER NOT NULL,
    algorithm      TEXT    NOT NULL,
    confirmed      INTEGER NOT NULL DEFAULT 0,
    created_at     INTEGER NOT NULL
);

CREATE TABLE duplicate_group_members (
    group_id  INTEGER NOT NULL REFERENCES duplicate_groups(id) ON DELETE CASCADE,
    entry_id  INTEGER NOT NULL,
    is_keeper INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (group_id, entry_id)
) WITHOUT ROWID;

-- Cleanup candidates. Every row carries its own reasoning and risk class; a
-- candidate without a reason is not a candidate.
CREATE TABLE cleanup_candidates (
    id                INTEGER PRIMARY KEY,
    scan_id           INTEGER NOT NULL,
    entry_id          INTEGER NOT NULL,
    rule              TEXT    NOT NULL,
    safety            INTEGER NOT NULL,  -- 0 safe 1 review 2 protected
    reason            TEXT    NOT NULL,
    reclaimable_bytes INTEGER NOT NULL DEFAULT 0,
    state             TEXT    NOT NULL DEFAULT 'proposed',
    created_at        INTEGER NOT NULL
);

CREATE INDEX idx_cleanup_scan ON cleanup_candidates(scan_id, safety, reclaimable_bytes DESC);

-- Point-in-time storage snapshots, and comparisons between two scans, for the
-- storage-change history feature.
CREATE TABLE snapshots (
    id            INTEGER PRIMARY KEY,
    volume_id     INTEGER REFERENCES volumes(id) ON DELETE CASCADE,
    scan_id       INTEGER,
    label         TEXT    NOT NULL,
    captured_at   INTEGER NOT NULL,
    total_bytes   INTEGER,
    used_bytes    INTEGER,
    indexed_bytes INTEGER NOT NULL DEFAULT 0,
    entry_count   INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE scan_comparisons (
    id              INTEGER PRIMARY KEY,
    base_scan_id    INTEGER NOT NULL,
    target_scan_id  INTEGER NOT NULL,
    created_at      INTEGER NOT NULL,
    bytes_added     INTEGER NOT NULL DEFAULT 0,
    bytes_removed   INTEGER NOT NULL DEFAULT 0,
    entries_added   INTEGER NOT NULL DEFAULT 0,
    entries_removed INTEGER NOT NULL DEFAULT 0,
    UNIQUE (base_scan_id, target_scan_id)
);
