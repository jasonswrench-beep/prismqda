/// Why an import failed. Messages are written for a person to read.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),

    /// The file is damaged, or is not what its name says it is.
    #[error("{0}")]
    Format(String),

    /// A kind of file, or a variant of one, that this version does not read.
    #[error("{0}")]
    Unsupported(String),

    #[error(transparent)]
    Engine(#[from] prismqda_engine::Error),
}

pub type Result<T> = std::result::Result<T, ImportError>;

impl ImportError {
    pub fn format(message: impl Into<String>) -> Self {
        ImportError::Format(message.into())
    }

    pub fn unsupported(message: impl Into<String>) -> Self {
        ImportError::Unsupported(message.into())
    }
}
