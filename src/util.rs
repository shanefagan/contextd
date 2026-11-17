//! Shared helpers.

use std::sync::{Mutex, MutexGuard, PoisonError};

/// Locks a mutex, recovering the data if a previous holder panicked.
///
/// Every piece of state contextd keeps behind a mutex is a cache or a
/// registry that stays consistent between statements, so a panic in one
/// request must not take every later request down with it.
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}
