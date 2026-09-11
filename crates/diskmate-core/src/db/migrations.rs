//! Forward-only schema migrations.
//!
//! Each migration is an embedded SQL file applied inside a transaction and
//! recorded in `schema_migrations`. Migrations are never edited after release;
//! a schema change is always a new file.

use rusqlite::Connection;

use crate::error::{CoreError, Result};

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial",
        sql: include_str!("migrations/001_initial.sql"),
    },
    Migration {
        version: 2,
        name: "analysis_extensions",
        sql: include_str!("migrations/002_analysis_extensions.sql"),
    },
];

/// Schema version this build expects.
pub const SCHEMA_VERSION: i64 = 2;

pub fn current_schema_version(conn: &Connection) -> Result<i64> {
    let exists: i64 = conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations'",
        [],
        |row| row.get(0),
    )?;
    if exists == 0 {
        return Ok(0);
    }
    let version: Option<i64> =
        conn.query_row("SELECT max(version) FROM schema_migrations", [], |row| row.get(0))?;
    Ok(version.unwrap_or(0))
}

pub fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
             version    INTEGER PRIMARY KEY,
             name       TEXT NOT NULL,
             applied_at INTEGER NOT NULL
         );",
    )?;

    let applied = current_schema_version(conn)?;
    for migration in MIGRATIONS.iter().filter(|m| m.version > applied) {
        conn.execute_batch("BEGIN")?;
        let result = conn.execute_batch(migration.sql).and_then(|_| {
            conn.execute(
                "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
                rusqlite::params![migration.version, migration.name, super::now_unix()],
            )
            .map(|_| ())
        });
        match result {
            Ok(()) => {
                conn.execute_batch("COMMIT")?;
            }
            Err(source) => {
                let _ = conn.execute_batch("ROLLBACK");
                return Err(CoreError::Migration { version: migration.version, source });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_apply_and_are_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        assert_eq!(current_schema_version(&conn).unwrap(), SCHEMA_VERSION);
        migrate(&conn).unwrap();
        assert_eq!(current_schema_version(&conn).unwrap(), SCHEMA_VERSION);
    }

    #[test]
    fn every_expected_table_exists() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        for table in [
            "volumes",
            "scans",
            "fs_entries",
            "scan_errors",
            "scan_benchmarks",
            "scan_categories",
            "scan_extensions",
            "file_classifications",
            "content_hashes",
            "duplicate_groups",
            "duplicate_group_members",
            "cleanup_candidates",
            "snapshots",
            "scan_comparisons",
        ] {
            let count: i64 = conn
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "missing table {table}");
        }
    }

    #[test]
    fn migration_versions_are_contiguous_and_ordered() {
        for (index, migration) in MIGRATIONS.iter().enumerate() {
            assert_eq!(migration.version, index as i64 + 1);
        }
        assert_eq!(MIGRATIONS.last().unwrap().version, SCHEMA_VERSION);
    }
}
