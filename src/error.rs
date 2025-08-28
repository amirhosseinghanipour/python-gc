use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum GCError {
    #[error("Object is already tracked")]
    AlreadyTracked,

    #[error("Object is not tracked")]
    NotTracked,

    #[error("Garbage collection already in progress")]
    CollectionInProgress,

    #[error("Invalid generation: {0}")]
    InvalidGeneration(usize),

    #[error("Object has finalizer and cannot be collected")]
    HasFinalizer,

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Memory allocation failed: {0}")]
    AllocationFailed(String),

    #[error("Reference count error: {0}")]
    ReferenceCountError(String),
}

impl From<std::io::Error> for GCError {
    fn from(err: std::io::Error) -> Self {
        GCError::Internal(format!("IO error: {err}"))
    }
}

impl From<std::alloc::LayoutError> for GCError {
    fn from(err: std::alloc::LayoutError) -> Self {
        GCError::AllocationFailed(format!("Layout error: {err}"))
    }
}
