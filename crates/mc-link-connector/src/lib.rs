//! Server connector implementations for various connection methods.
//!
//! This crate provides concrete implementations of the [`ServerConnector`] trait
//! for different connection methods like local filesystem access, FTP, SSH, etc.

#![cfg_attr(not(debug_assertions), forbid(missing_docs))]

pub mod connectors;
pub mod prelude;

pub use connectors::*;
