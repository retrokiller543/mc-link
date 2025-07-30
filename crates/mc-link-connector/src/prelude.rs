//! Prelude module for connector implementations.
//!
//! Import with `use mc_link_connector::prelude::*;` to get commonly used connector types.

pub use crate::connectors::local::LocalConnector;
pub use crate::connectors::ftp::FtpConnector;
pub use mc_link_core::prelude::*;