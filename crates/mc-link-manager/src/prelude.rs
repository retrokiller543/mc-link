//! Prelude module for Minecraft server management.
//!
//! Import with `use mc_link_manager::prelude::*;` to get commonly used manager types.

pub use crate::actions::{SyncAction, SyncPlan, SyncSummary, SyncTarget};
pub use crate::error::{ManagerError, Result};
pub use crate::manager::MinecraftManager;
pub use crate::structure::{
    MinecraftStructure, ModsStructure, ConfigStructure, 
    ResourcePackStructure, ShaderPackStructure,
};

// Re-export commonly used types from dependencies
pub use mc_link_core::prelude::*;
pub use mc_link_compat::prelude::*;