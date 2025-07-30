//! Prelude module for configuration management.
//!
//! Import with `use mc_link_config::prelude::*;` to get commonly used config types.

// Core types
pub use crate::{ConfigManager, PROJECT_DIRS, CONFIG_MANAGER};

// Error handling
pub use crate::error::{ConfigError, Result};

// Server configuration
pub use crate::servers::{
    ConnectionType, ModLoader, ServerConfig, ServersConfig,
    LocalConnection, FtpConnection, SshConnection,
    ServerSettings, CompatibilityConfig, GlobalServerSettings,
};

// Profile management
pub use crate::profiles::{
    ProfileManager, CompatibilityProfile, CompatibilityRule,
    ProfileType, RuleAction, ProfileIndex,
};

// Configuration macros
pub use crate::{config_struct, config_enum, config_accessors};