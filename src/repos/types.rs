use failure::Error as FailureError;
use futures::future::Future;

/// Repos layer Future
pub type RepoResult<T> = Result<T, FailureError>;
