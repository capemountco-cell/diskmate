//! DiskMate core engine.
//!
//! This crate contains every piece of DiskMate that is not presentation:
//! filesystem scanning, the local SQLite index, aggregation queries, and the
//! platform abstraction used for volume, memory and startup inspection.
//!
//! It deliberately has **no dependency on Tauri**. The Tauri layer is a thin
//! adapter that maps commands and events onto the services exposed here, which
//! keeps the engine testable on any Unix host and portable to other platforms
//! later.
//!
//! # Safety posture
//!
//! The engine is **read-only**. There is no code path in this crate that
//! deletes, moves, or modifies a file on the user's disk. Destructive
//! operations are deferred to a future centralised cleanup service (see
//! `docs/security.md`).

pub mod db;
pub mod error;
pub mod model;
pub mod paths;
pub mod platform;
pub mod scanner;
pub mod services;

pub use error::{CoreError, Result};

/// Version of the scanning engine, recorded on every scan row so that
/// benchmark results and stored scans can be attributed to a revision.
pub const ENGINE_VERSION: &str = concat!("diskmate-core/", env!("CARGO_PKG_VERSION"));
