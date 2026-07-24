//! Shared failure boundary for public `infra-core` operations.
//!
//! The enum provides transparent I/O, JSON, and CSV source variants for uncontextualized
//! conversions. Callers may instead wrap failures with reader-facing context in [`Error::Msg`]
//! through [`Error::msg`]. The crate does not currently expose stable machine-readable domain
//! error codes.

use thiserror::Error;

/// Result type returned by fallible `infra-core` operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors surfaced by data loading, parsing, and domain validation.
#[derive(Debug, Error)]
pub enum Error {
    /// Domain, unsupported-state, or contextualized failure with reader-facing context.
    #[error("{0}")]
    Msg(String),
    /// Filesystem or stream failure.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// JSON decoding or encoding failure.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// CSV decoding or encoding failure.
    #[error(transparent)]
    Csv(#[from] csv::Error),
}

impl Error {
    /// Construct a domain, unsupported-state, or contextualized error.
    pub fn msg(s: impl Into<String>) -> Self {
        Self::Msg(s.into())
    }
}
