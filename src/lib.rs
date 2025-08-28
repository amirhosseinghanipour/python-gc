//! Python Garbage Collector Implementation in Rust
//!
//! This is a Rust implementation of Python's reference counting garbage collector
//! with cycle detection. It provides the core functionality for managing object
//! lifecycles and detecting reference cycles.

pub mod collector;
pub mod error;
pub mod ffi;
pub mod gc;
pub mod generation;
pub mod object;
pub mod traversal;

pub use error::GCError;
pub use gc::GarbageCollector;
pub use object::{ObjectId, PyGCHead, PyObject};

pub type GCResult<T> = Result<T, GCError>;
