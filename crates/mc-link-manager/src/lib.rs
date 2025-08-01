//! High-level Minecraft server management with connector abstraction.
//!
//! This crate provides a `MinecraftManager` that wraps server connectors and
//! handles comparison, synchronization, and management of Minecraft server files.

#![cfg_attr(not(debug_assertions), forbid(missing_docs))]

pub mod actions;
pub mod error;
pub mod manager;
pub mod prelude;
pub mod scanning;
pub mod structure;

pub use actions::*;
pub use error::*;
pub use manager::*;
pub use structure::*;
