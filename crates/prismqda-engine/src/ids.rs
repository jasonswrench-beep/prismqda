//! Ids that are never given again (project format 16, `schema/v16.sql`).
//!
//! A code or a case keeps its number for as long as the project exists. SQLite would hand the number of the newest row out again after
//! that row was removed, and Undo of "add a code" or "add a case" removes it; a file that names codes and cases by number (the case
//! table sent to PrismStatistics) would then mean something else the next time it was made. So a new code or case does not take the
//! number SQLite would choose: it takes the one above the highest number ever given, which `id_floor` remembers.

use rusqlite::{params, Connection};

use crate::Result;

/// The kinds of row whose numbers are never given again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Code,
    Case,
}

impl Kind {
    fn table(self) -> &'static str {
        match self {
            Kind::Code => "code",
            Kind::Case => "research_case",
        }
    }
}

/// The number the next code or case takes, and remembers it, so nothing takes it again. Call it inside the transaction that adds the row.
pub(crate) fn next_id(conn: &Connection, kind: Kind) -> Result<i64> {
    let table = kind.table();
    // `table` is one of the two names above, never text from outside, so it is safe to put in the statement.
    let held: i64 = conn.query_row(&format!("SELECT COALESCE(MAX(id), 0) FROM {table}"), [], |r| r.get(0))?;
    let given: i64 = conn.query_row("SELECT COALESCE((SELECT last_id FROM id_floor WHERE table_name = ?1), 0)", [table], |r| r.get(0))?;
    let id = held.max(given) + 1;
    conn.execute(
        "INSERT INTO id_floor (table_name, last_id) VALUES (?1, ?2)
         ON CONFLICT (table_name) DO UPDATE SET last_id = excluded.last_id",
        params![table, id],
    )?;
    Ok(id)
}
