//! Core abstractions and types for Minecraft server management.
//!
//! This crate provides the fundamental building blocks for connecting to and managing
//! Minecraft servers across different platforms and connection methods.

#![cfg_attr(not(debug_assertions), forbid(missing_docs))]

pub mod error;
pub mod prelude;
pub mod server;
pub mod logging;

pub use error::*;
pub use server::*;
