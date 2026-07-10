//! Golden-vector tests against the C++ oracle. See tests/.
//!
//! Also hosts the [`scenario`] parser: the single source of truth for the
//! Slice-2 physics scenario file, read by both the Rust differential test and
//! (eventually) the C++ dumper. Kept here because only `oracle-tests` reads it.

pub mod scenario;
