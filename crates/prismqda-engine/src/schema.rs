//! The project file's schema, its version, and the way older files are brought forward.

use rusqlite::Connection;

use crate::Result;

/// Marks a SQLite file as a PrismQDA project. The bytes spell "PQDA".
pub const APPLICATION_ID: i32 = 0x5051_4441;

/// The full schema at version 1, published in the repository as `schema/v1.sql`.
pub const INITIAL_SCHEMA: &str = include_str!("../schema/v1.sql");

/// Each entry upgrades a file by one version: entry 0 takes version 1 to 2, entry 1 takes 2 to 3, and so on.
/// Each is also published as a file beside `v1.sql`, named for the version it produces.
pub const MIGRATIONS: &[&str] = &[
    // Version 2: a coding can carry a 1 to 5 rating. Published as `schema/v2.sql`.
    include_str!("../schema/v2.sql"),
    // Version 3: code properties (exclusive group, rating scale) and counterexamples. Published as `schema/v3.sql`.
    include_str!("../schema/v3.sql"),
    // Version 4: saved and frozen queries, and concept maps. Published as `schema/v4.sql`.
    include_str!("../schema/v4.sql"),
    // Version 5: AI suggestible codes and the project's AI setting. Published as `schema/v5.sql`.
    include_str!("../schema/v5.sql"),
    // Version 6: recordings linked to documents, and the time a unit is spoken. Published as `schema/v6.sql`.
    include_str!("../schema/v6.sql"),
    // Version 7: images kept in the project, and coded areas of an image. Published as `schema/v7.sql`.
    include_str!("../schema/v7.sql"),
    // Version 8: adjudication, the decisions that settle a unit coders coded differently. Published as `schema/v8.sql`.
    include_str!("../schema/v8.sql"),
    // Version 9: the Statistics notebook kept in the project (pages, notes, and earlier runs). Published as `schema/v9.sql`.
    include_str!("../schema/v9.sql"),
    // Version 10: Mixed Methods kept in the project (the design, the triangulation matrix, and the integration log). Published as `schema/v10.sql`.
    include_str!("../schema/v10.sql"),
    // Version 11: PDFs kept in the project, and coded areas of a PDF page. Published as `schema/v11.sql`.
    include_str!("../schema/v11.sql"),
    // Version 12: recordings imported without captions, and coded stretches (spans) of a recording. Published as `schema/v12.sql`.
    include_str!("../schema/v12.sql"),
    // Version 13: the list of names to pseudonymize or redact, and the switch that applies it to everything that is exported. Published as `schema/v13.sql`.
    include_str!("../schema/v13.sql"),
    // Version 14: where a theme stands in its development (candidate, reviewed, defined). Published as `schema/v14.sql`.
    include_str!("../schema/v14.sql"),
    // Version 15: deciding where coders differ on an area or a stretch. Published as `schema/v15.sql`.
    include_str!("../schema/v15.sql"),
];

/// The schema version this build writes and understands.
pub const SCHEMA_VERSION: i32 = 1 + MIGRATIONS.len() as i32;

/// The whole schema as this build writes it, for people who read a project file with R, Python, or any
/// SQLite tool: version 1 as published, then each step that follows it, in order.
pub fn published_schema() -> String {
    let mut text = INITIAL_SCHEMA.to_string();
    for (i, step) in MIGRATIONS.iter().enumerate() {
        if !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&format!("\n-- ==== The step from version {} to version {}, run after the version 1 tables above ====\n\n", i + 1, i + 2));
        text.push_str(step);
    }
    text
}

/// Builds a new project file's tables: version 1, then every migration, in one transaction.
pub(crate) fn install(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute_batch(INITIAL_SCHEMA)?;
    for step in MIGRATIONS {
        tx.execute_batch(step)?;
    }
    tx.pragma_update(None, "application_id", APPLICATION_ID)?;
    tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    tx.commit()?;
    Ok(())
}

/// Brings a file at version `from` up to `1 + migrations.len()`, one version per step, all in one
/// transaction so a failure leaves the file exactly as it was. Returns the version reached.
pub(crate) fn migrate(conn: &mut Connection, from: i32, migrations: &[&str]) -> Result<i32> {
    let target = 1 + migrations.len() as i32;
    if from >= target {
        return Ok(from);
    }
    let tx = conn.transaction()?;
    for version in from..target {
        tx.execute_batch(migrations[(version - 1) as usize])?;
        tx.pragma_update(None, "user_version", version + 1)?;
    }
    tx.commit()?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user_version(conn: &Connection) -> i32 {
        conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap()
    }

    #[test]
    fn migrate_applies_each_step_in_order() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t (a INTEGER);").unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        let steps = ["ALTER TABLE t ADD COLUMN b INTEGER;", "ALTER TABLE t ADD COLUMN c INTEGER;"];
        assert_eq!(migrate(&mut conn, 1, &steps).unwrap(), 3);
        assert_eq!(user_version(&conn), 3);
        conn.execute("INSERT INTO t (a, b, c) VALUES (1, 2, 3)", []).unwrap();
    }

    #[test]
    fn migrate_starts_from_the_files_own_version() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t (a INTEGER, b INTEGER);").unwrap();
        conn.pragma_update(None, "user_version", 2).unwrap();
        // The first step must be skipped: running it again would fail on a duplicate column.
        let steps = ["ALTER TABLE t ADD COLUMN b INTEGER;", "ALTER TABLE t ADD COLUMN c INTEGER;"];
        assert_eq!(migrate(&mut conn, 2, &steps).unwrap(), 3);
        assert_eq!(user_version(&conn), 3);
    }

    #[test]
    fn a_failing_step_leaves_the_file_unchanged() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t (a INTEGER);").unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        let steps = ["ALTER TABLE t ADD COLUMN b INTEGER;", "THIS IS NOT SQL;"];
        assert!(migrate(&mut conn, 1, &steps).is_err());
        assert_eq!(user_version(&conn), 1);
        // The first step was rolled back with the second.
        assert!(conn.execute("INSERT INTO t (a, b) VALUES (1, 2)", []).is_err());
    }

    #[test]
    fn nothing_to_do_when_already_current() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        assert_eq!(migrate(&mut conn, 1, &[]).unwrap(), 1);
    }

    #[test]
    fn schema_version_matches_the_migration_list() {
        assert_eq!(SCHEMA_VERSION, 1 + MIGRATIONS.len() as i32);
    }
}
