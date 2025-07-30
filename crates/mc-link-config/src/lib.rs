//! Minecraft server management configuration system.
//! 
//! This module provides a unified configuration system for managing Minecraft servers,
//! including server definitions, connection details, compatibility profiles, and
//! extensible resource management for templates and profiles.

#![cfg_attr(not(debug_assertions), forbid(missing_docs))]

pub mod error;
pub mod macros;
pub mod servers;
pub mod profiles;
pub mod prelude;
pub mod manager;

use directories::ProjectDirs;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::sync::LazyLock;

pub use error::*;
pub use servers::*;
pub use profiles::*;
pub use manager::{ManagerConfig, LogFileNameFormat};

/// Project directories for mc-link.
pub static PROJECT_DIRS: LazyLock<ProjectDirs> = LazyLock::new(|| {
    if let Some(dirs) = ProjectDirs::from("com", "tosic", "mc-link") {
        dirs
    } else {
        eprintln!("Failed to determine project directories. Ensure your environment supports directories.");
        exit(1);
    }
});

/// Global configuration manager instance.
pub static CONFIG_MANAGER: LazyLock<ConfigManager> = LazyLock::new(|| {
    match ConfigManager::new() {
        Ok(manager) => manager,
        Err(e) => {
            eprintln!("Failed to initialize configuration manager: {}", e);
            exit(1);
        }
    }
});

/// Main configuration manager for Minecraft server management.
#[derive(Debug, Clone)]
pub struct ConfigManager {
    pub manager: ManagerConfig,
    servers_config: ServersConfig,
    profile_manager: ProfileManager,
    config_dir: PathBuf,
}

impl ConfigManager {
    /// Creates a new configuration manager with default directory.
    pub fn new() -> Result<Self> {
        Self::from_dir(None)
    }

    /// Creates a configuration manager from a specific directory.
    pub fn from_dir(config_dir: Option<&Path>) -> Result<Self> {
        let config_dir = config_dir
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PROJECT_DIRS.config_dir().to_path_buf());

        // Ensure the config directory exists
        std::fs::create_dir_all(&config_dir)
            .map_err(|e| ConfigError::io_error(
                "create config directory",
                format!("Failed to create config directory: {}", e),
                Some(e)
            ))?;

        let manager = ManagerConfig::load(&config_dir)?;
        // Load servers configuration
        let servers_config = ServersConfig::load(&config_dir)?;

        // Initialize profile manager
        let profile_manager = ProfileManager::new(&config_dir)?;

        Ok(Self {
            manager,
            servers_config,
            profile_manager,
            config_dir,
        })
    }
    
    /// Gets a reference to the manager configuration.
    pub fn manager(&self) -> &ManagerConfig {
        &self.manager
    }
    
    /// Gets a mutable reference to the manager configuration.
    pub fn manager_mut(&mut self) -> &mut ManagerConfig {
        &mut self.manager
    }

    /// Gets a reference to the servers configuration.
    pub fn servers(&self) -> &ServersConfig {
        &self.servers_config
    }

    /// Gets a mutable reference to the servers configuration.
    pub fn servers_mut(&mut self) -> &mut ServersConfig {
        &mut self.servers_config
    }

    /// Gets a reference to the profile manager.
    pub fn profiles(&self) -> &ProfileManager {
        &self.profile_manager
    }

    /// Gets a mutable reference to the profile manager.
    pub fn profiles_mut(&mut self) -> &mut ProfileManager {
        &mut self.profile_manager
    }

    /// Gets the configuration directory path.
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    /// Saves all configuration to disk.
    pub fn save(&self) -> Result<()> {
        self.manager.save(&self.config_dir)?;
        self.servers_config.save(&self.config_dir)?;
        self.profile_manager.save()?;
        Ok(())
    }

    /// Reloads configuration from disk.
    pub fn reload(&mut self) -> Result<()> {
        self.servers_config = ServersConfig::load(&self.config_dir)?;
        self.profile_manager.reload()?;
        Ok(())
    }

    /// Gets a server configuration by ID.
    pub fn get_server(&self, server_id: &str) -> Option<&ServerConfig> {
        self.servers_config.servers.get(server_id)
    }

    /// Adds or updates a server configuration.
    pub fn add_server(&mut self, server: ServerConfig) {
        self.servers_config.servers.insert(server.id.clone(), server);
    }

    /// Removes a server configuration.
    pub fn remove_server(&mut self, server_id: &str) -> Option<ServerConfig> {
        self.servers_config.servers.remove(server_id)
    }

    /// Lists all server IDs.
    pub fn list_servers(&self) -> Vec<&str> {
        self.servers_config.servers.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new().expect("Failed to create default config manager")
    }
}