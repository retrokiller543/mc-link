//! Prelude module with commonly used types and traits.
//!
//! This module re-exports the most commonly used items from the crate,
//! allowing users to quickly import everything they need with `use mc_link_core::prelude::*;`.

pub use crate::cache::{CacheStats, CachedJarInfo, GlobalJarCache, ServerStructureCache};
pub use crate::error::{CoreError, Result};
pub use crate::progress::{ProgressAware, ProgressReporter, ProgressStage, ProgressUpdate};
pub use crate::server::{ModInfo, ProgressCallback, ServerConnector, ServerInfo, ServerStatus};
