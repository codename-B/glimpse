//! Provides a COM abstraction layer for safe, boilerplate-free COM implementations.
//!
//! This module provides helpers and macros that eliminate repetitive patterns
//! in COM interface implementations while ensuring panic-free operation
//! (critical for DLLs loaded by Explorer).
//!
//! # Overview
//!
//! - [`helpers::ComWrapper`] - The COM object wrapper structure
//! - [`helpers::MutexExt`] - Panic-free mutex locking
//! - [`com_method!`] - Macro for COM method implementations
//! - [`define_vtable!`] - Macro for VTable generation
//!
//! # Examples
//! ```
//! use std::sync::Mutex;
//!
//! use glimpse::com::MutexExt;
//!
//! let mutex = Mutex::new(7u32);
//! let guard = mutex.lock_or_fail().expect("lock should succeed");
//! assert_eq!(*guard, 7);
//! ```

pub mod helpers;

#[macro_use]
pub mod macros;

// Re-export commonly used items
pub use helpers::{ComWrapper, MutexExt};
