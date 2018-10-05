use failure::Error as FailureError;

/// Repos layer Future
pub type RepoResult<T> = Result<T, FailureError>;
