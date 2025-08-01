//! Core abstractions and types for Minecraft server management.
//!
//! This crate provides the fundamental building blocks for connecting to and managing
//! Minecraft servers across different platforms and connection methods.

#![cfg_attr(not(debug_assertions), forbid(missing_docs))]

pub mod cache;
pub mod error;
pub mod logging;
pub mod prelude;
pub mod progress;
pub mod server;
pub mod traits;

pub use cache::{CacheStats, CachedJarInfo, GlobalJarCache, ServerStructureCache};
pub use error::*;
pub use progress::*;
pub use server::*;
