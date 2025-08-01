//! Mod compatibility checking and analysis for Minecraft.
//!
//! This crate provides functionality to compare mod installations between
//! client and server, detect compatibility issues, and manage ignore lists.

#![cfg_attr(not(debug_assertions), forbid(missing_docs))]

pub mod error;
pub mod jar;
pub mod prelude;
pub mod rules;

pub use error::*;
pub use jar::*;
pub use rules::*;
