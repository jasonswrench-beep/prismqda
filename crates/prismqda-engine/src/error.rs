use std::path::PathBuf;

/// Everything the engine can refuse or fail at. Messages are written for a person to read.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {}", describe(.0))]
    Database(#[from] rusqlite::Error),

    #[error("file error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("{} is not a PrismQDA project", .0.display())]
    NotAProject(PathBuf),

    #[error("{} already exists, so nothing was created", .0.display())]
    AlreadyExists(PathBuf),

    #[error("this project is open read-only: {0}")]
    ReadOnly(String),

    #[error("{0}")]
    Invalid(String),

    /// The project is encrypted and no passphrase was given.
    #[error("this project is encrypted: type its passphrase to open it")]
    NeedsPassphrase(PathBuf),

    /// The passphrase does not open the project (or the file is not a PrismQDA project at all).
    #[error("that passphrase does not open this project (or the file is not a PrismQDA project)")]
    WrongPassphrase,

    #[error("{what} {id} does not exist")]
    NotFound { what: &'static str, id: i64 },

    /// A mutually exclusive group would be broken: the passage already has another code of the group. The
    /// message names the group and the code, so it can be shown as it is, and the caller may try again
    /// asking to replace.
    #[error("{0}")]
    ExclusiveGroup(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// SQLite's message plus its extended result code, so an odd failure (a disk I/O error, say) can
/// be traced to the exact cause instead of being a bare "disk I/O error".
fn describe(e: &rusqlite::Error) -> String {
    match e {
        rusqlite::Error::SqliteFailure(failure, _) => format!("{e} (SQLite code {})", failure.extended_code),
        other => other.to_string(),
    }
}
