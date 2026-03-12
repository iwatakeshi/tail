//! Tail operations module.
//!
//! Provides line-based, byte-based, and follow-mode tail operations
//! for both seekable files and non-seekable streams.

pub mod bytes;
pub mod follow;
pub mod lines;
