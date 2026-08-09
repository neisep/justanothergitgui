//! Explicit failure types for the git adapters.
//!
//! These exist so a caller can tell *what* went wrong. A flattened `String`
//! forces every layer above to either re-parse prose or treat "this repository
//! is bare", "the file has broken markers" and "the disk is full" as the same
//! event — which is exactly the distinction the merge editor needs to decide
//! between an error banner, a retry, and a fallback path.

use std::fmt;

/// What can go wrong reading a conflicted file, or writing its resolution back.
#[derive(Debug)]
pub enum ConflictError {
    /// The repository has no working tree, so there is no file to read or write.
    BareRepository,
    /// libgit2 refused an index, blob, or staging call.
    Git(git2::Error),
    /// The working-tree file could not be read or written.
    Io(std::io::Error),
    /// The file's `<<<<<<<` / `=======` / `>>>>>>>` markers do not pair up, so
    /// there is no way to tell which side a line belongs to.
    UnbalancedMarkers,
}

impl fmt::Display for ConflictError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BareRepository => write!(formatter, "Bare repositories are not supported"),
            Self::Git(error) => write!(formatter, "{error}"),
            Self::Io(error) => write!(formatter, "{error}"),
            Self::UnbalancedMarkers => write!(formatter, "Unbalanced conflict markers"),
        }
    }
}

impl std::error::Error for ConflictError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Git(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::BareRepository | Self::UnbalancedMarkers => None,
        }
    }
}

impl From<git2::Error> for ConflictError {
    fn from(error: git2::Error) -> Self {
        Self::Git(error)
    }
}

impl From<std::io::Error> for ConflictError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

#[cfg(test)]
mod tests {
    use super::ConflictError;

    #[test]
    fn each_variant_describes_itself() {
        assert_eq!(
            ConflictError::UnbalancedMarkers.to_string(),
            "Unbalanced conflict markers"
        );
        assert_eq!(
            ConflictError::BareRepository.to_string(),
            "Bare repositories are not supported"
        );

        let io = ConflictError::Io(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "denied",
        ));
        assert_eq!(io.to_string(), "denied");
    }

    #[test]
    fn wrapped_errors_stay_reachable_as_a_source() {
        let io = ConflictError::Io(std::io::Error::other("boom"));
        assert!(std::error::Error::source(&io).is_some());
        assert!(std::error::Error::source(&ConflictError::UnbalancedMarkers).is_none());
    }
}
