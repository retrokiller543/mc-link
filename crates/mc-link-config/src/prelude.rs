//! Prelude module for configuration management.
//!
//! Import with `use mc_link_config::prelude::*;` to get commonly used config types.

// Core types
pub use crate::{CONFIG_MANAGER, ConfigManager, PROJECT_DIRS};

// Error handling
pub use crate::error::{ConfigError, Result};

// Server configuration
pub use crate::servers::{
    CompatibilityConfig, ConnectionType, FtpConnection, GlobalServerSettings, LocalConnection,
    ModLoader, ServerConfig, ServerSettings, ServersConfig, SshConnection,
};

// Profile management
pub use crate::profiles::{
    CompatibilityProfile, CompatibilityRule, ProfileIndex, ProfileManager, ProfileType, RuleAction,
};

// Configuration macros
pub use crate::{config_accessors, config_enum, config_struct};
