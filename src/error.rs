use crate::adapter::ErrorKind;

#[derive(Debug, thiserror::Error)]
pub enum PolycodeError {
    #[error("adapter error: {0}")]
    Adapter(#[from] ErrorKind),

    #[error("no adapter available: {0}")]
    NoAdapter(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
