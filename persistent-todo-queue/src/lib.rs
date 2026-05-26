//! Persistent FIFO todo queue.
//!
//! - [`Todo`] is the task struct.
//! - [`Queue<T>`] is a generic FIFO queue backed by [`std::collections::VecDeque`].
//! - [`Store`] reads/writes a [`Queue<T>`] to a Borsh-encoded file with atomic rename.

pub mod queue;
pub mod store;
pub mod todo;

pub use queue::Queue;
pub use store::Store;
pub use todo::Todo;

use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn next_id(queue: &Queue<Todo>) -> u64 {
    queue.iter().map(|t| t.id).max().unwrap_or(0) + 1
}
