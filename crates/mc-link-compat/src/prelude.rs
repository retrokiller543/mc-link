//! Prelude module for mod compatibility checking.
//!
//! Import with `use mc_link_compat::prelude::*;` to get commonly used compatibility types.

pub use crate::error::{CompatError, Result};
pub use crate::jar::extract_jar_info;
pub use mc_link_core::{ModInfo, ModLoader, ModSide};
pub use crate::rules::{
    CompatConfig, CompatResult, CompatRule, RuleType, VersionMismatch, check_compatibility,
};