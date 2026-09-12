-- Milestone 02: cleanup, duplicates, the app uninstaller and snapshots.
--
-- Migration 002 reserved the shape of these tables before anything used them.
-- Now that they are used, the columns the features actually need are added
-- here rather than by editing a released migration.

-- `cleanup_candidates` is rebuilt rather than altered.
--
-- Migration 002 reserved the table with `entry_id` NOT NULL, on the assumption
-- that every candidate comes from a scanned entry. The uninstaller breaks that
-- assumption: an application's leftovers are found by looking where that
-- application scatters files, not by reading a scan, so they have no entry to
-- point at. SQLite cannot relax a NOT NULL in place, and the table has never
-- held a row, so recreating it is both correct and free.
--
-- It also gains what a staged queue actually needs: the path itself (a
-- candidate outlives the scan that proposed it, and entry paths are
-- reconstructed), what kind of thing it is, the size as it was when staged so
-- removal can detect a change, and the timings.
DROP TABLE cleanup_candidates;

CREATE TABLE cleanup_candidates (
    id                INTEGER PRIMARY KEY,
    scan_id           INTEGER NOT NULL,
    -- Null for anything found outside a scan, such as application leftovers.
    entry_id          INTEGER,
    path              TEXT    NOT NULL,
    kind              TEXT    NOT NULL DEFAULT 'other',
    rule              TEXT    NOT NULL,
    safety            INTEGER NOT NULL,  -- 0 safe 1 review 2 protected
    reason            TEXT    NOT NULL,
    detail            TEXT,
    reclaimable_bytes INTEGER NOT NULL DEFAULT 0,
    modified_at       INTEGER,
    state             TEXT    NOT NULL DEFAULT 'proposed',
    -- The size as reviewed. Removal re-checks it: a file that changed after
    -- being staged is refused rather than trashed on stale information.
    staged_size       INTEGER,
    staged_at         INTEGER,
    resolved_at       INTEGER,
    created_at        INTEGER NOT NULL,
    -- Groups leftovers with the application they belong to.
    group_key         TEXT
);

CREATE INDEX idx_cleanup_scan ON cleanup_candidates(scan_id, safety, reclaimable_bytes DESC);
CREATE INDEX idx_cleanup_state ON cleanup_candidates(state, reclaimable_bytes DESC);
CREATE INDEX idx_cleanup_group ON cleanup_candidates(group_key) WHERE group_key IS NOT NULL;
CREATE UNIQUE INDEX idx_cleanup_path ON cleanup_candidates(scan_id, path, rule);

-- Every destructive action DiskMate has ever taken, whether it succeeded or
-- not. This is an audit log, not a cache: it is never pruned with its scan,
-- because "what did this app do to my Mac" has to be answerable later.
CREATE TABLE cleanup_operations (
    id            INTEGER PRIMARY KEY,
    candidate_id  INTEGER,
    path          TEXT    NOT NULL,
    kind          TEXT    NOT NULL,
    safety        INTEGER NOT NULL,
    rule          TEXT    NOT NULL,
    reason        TEXT    NOT NULL,
    bytes         INTEGER NOT NULL DEFAULT 0,
    outcome       TEXT    NOT NULL,   -- trashed | refused | failed | vanished
    -- Why a refusal happened, in the words the user was shown.
    detail        TEXT,
    performed_at  INTEGER NOT NULL
);

CREATE INDEX idx_cleanup_ops_time ON cleanup_operations(performed_at DESC);

-- Installed applications and the files they scattered. Rebuilt on demand
-- rather than tied to a scan, because an uninstall is about the machine's
-- current state and not about a snapshot of it.
CREATE TABLE applications (
    id                INTEGER PRIMARY KEY,
    bundle_id         TEXT,
    name              TEXT    NOT NULL,
    path              TEXT    NOT NULL UNIQUE,
    version           TEXT,
    bundle_bytes      INTEGER NOT NULL DEFAULT 0,
    leftover_bytes    INTEGER NOT NULL DEFAULT 0,
    leftover_count    INTEGER NOT NULL DEFAULT 0,
    last_used_at      INTEGER,
    is_os_provided    INTEGER NOT NULL DEFAULT 0,
    scanned_at        INTEGER NOT NULL
);

CREATE TABLE application_leftovers (
    id             INTEGER PRIMARY KEY,
    application_id INTEGER NOT NULL REFERENCES applications(id) ON DELETE CASCADE,
    path           TEXT    NOT NULL,
    location       TEXT    NOT NULL,  -- caches | preferences | application support | ...
    bytes          INTEGER NOT NULL DEFAULT 0,
    safety         INTEGER NOT NULL,
    reason         TEXT    NOT NULL,
    UNIQUE (application_id, path)
);

-- `snapshots` gains what a comparison needs to be read back without the scan
-- it came from still being present.
ALTER TABLE snapshots ADD COLUMN root_path TEXT NOT NULL DEFAULT '';
ALTER TABLE snapshots ADD COLUMN file_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE snapshots ADD COLUMN dir_count INTEGER NOT NULL DEFAULT 0;

-- One row per directory that differs between two scans.
CREATE TABLE snapshot_deltas (
    id             INTEGER PRIMARY KEY,
    comparison_id  INTEGER NOT NULL REFERENCES scan_comparisons(id) ON DELETE CASCADE,
    path           TEXT    NOT NULL,
    name           TEXT    NOT NULL,
    change         TEXT    NOT NULL,  -- grew | shrank | added | removed
    before_bytes   INTEGER NOT NULL DEFAULT 0,
    after_bytes    INTEGER NOT NULL DEFAULT 0,
    delta_bytes    INTEGER NOT NULL
);

CREATE INDEX idx_snapshot_deltas ON snapshot_deltas(comparison_id, delta_bytes DESC);
