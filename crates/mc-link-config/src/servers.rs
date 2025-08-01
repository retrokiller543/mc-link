//! Server configuration definitions and management.

use crate::error::{ConfigError, Result};
use crate::{config_accessors, config_enum, config_struct};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

config_enum! {
    /// Supported mod loader types for Minecraft servers.
    pub enum ModLoader {
        NeoForge,
        Fabric,
        Forge,
        Vanilla,
        Unknown,
    }
    default = NeoForge
}

/// Connection type for server access with embedded configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "config")]
pub enum ConnectionType {
    /// Local filesystem connection
    #[serde(rename = "local")]
    Local(LocalConnection),
    /// FTP connection
    #[serde(rename = "ftp")]
    Ftp(FtpConnection),
    /// SSH connection
    #[serde(rename = "ssh")]
    Ssh(SshConnection),
    /// SFTP connection (uses SSH config)
    #[serde(rename = "sftp")]
    Sftp(SshConnection),
}

impl From<ConnectionType> for config::Value {
    fn from(val: ConnectionType) -> Self {
        use config::{Map, Value, ValueKind};
        use std::collections::HashMap;

        let (type_name, config_value) = match val {
            ConnectionType::Local(config) => ("local", config.into()),
            ConnectionType::Ftp(config) => ("ftp", config.into()),
            ConnectionType::Ssh(config) => ("ssh", config.into()),
            ConnectionType::Sftp(config) => ("sftp", config.into()),
        };

        let mut map = HashMap::new();
        map.insert(
            "type".to_string(),
            Value::new(None, ValueKind::String(type_name.to_string())),
        );
        map.insert("config".to_string(), config_value);

        Value::new(None, ValueKind::Table(Map::from_iter(map)))
    }
}

impl Default for ConnectionType {
    fn default() -> Self {
        Self::Local(LocalConnection::default())
    }
}

impl ConnectionType {
    /// Returns the connection type name as a string.
    pub fn type_name(&self) -> &'static str {
        match self {
            ConnectionType::Local(_) => "Local",
            ConnectionType::Ftp(_) => "FTP",
            ConnectionType::Ssh(_) => "SSH",
            ConnectionType::Sftp(_) => "SFTP",
        }
    }

    /// Returns true if this is a local connection.
    pub fn is_local(&self) -> bool {
        matches!(self, ConnectionType::Local(_))
    }

    /// Returns true if this is a remote connection.
    pub fn is_remote(&self) -> bool {
        !self.is_local()
    }

    /// Gets the host for remote connections.
    pub fn get_host(&self) -> Option<&str> {
        match self {
            ConnectionType::Local(_) => None,
            ConnectionType::Ftp(ftp) => Some(&ftp.host),
            ConnectionType::Ssh(ssh) | ConnectionType::Sftp(ssh) => Some(&ssh.host),
        }
    }

    /// Gets the username for connections that require authentication.
    pub fn get_username(&self) -> Option<&str> {
        match self {
            ConnectionType::Local(_) => None,
            ConnectionType::Ftp(ftp) => Some(&ftp.username),
            ConnectionType::Ssh(ssh) | ConnectionType::Sftp(ssh) => Some(&ssh.username),
        }
    }
}

config_struct! {
    /// Local filesystem connection configuration.
    pub struct LocalConnection {
        /// Path to the server directory
        pub path: String = String::new(),
    }
}

config_struct! {
    /// FTP connection configuration.
    pub struct FtpConnection {
        /// FTP server hostname
        pub host: String = String::new(),
        /// FTP server port
        pub port: u16 = 21,
        /// Username for authentication
        pub username: String = String::new(),
        /// Password (consider using environment variables)
        pub password: Option<String> = None,
        /// Base path on the server
        pub base_path: String = "/".to_string(),
        /// Use passive mode
        pub passive_mode: bool = true,
    }
}

config_struct! {
    /// SSH/SFTP connection configuration.
    pub struct SshConnection {
        /// SSH server hostname
        pub host: String = String::new(),
        /// SSH server port
        pub port: u16 = 22,
        /// Username for authentication
        pub username: String = String::new(),
        /// Path to private key file
        pub private_key_path: Option<String> = None,
        /// Password (if not using key auth)
        pub password: Option<String> = None,
        /// Base path on the server
        pub base_path: String = "/".to_string(),
    }
}

config_struct! {
    /// Server-specific settings and metadata.
    pub struct ServerSettings {
        /// Minecraft version (e.g., "1.20.1")
        pub minecraft_version: Option<String> = None,
        /// Mod loader type
        pub mod_loader: ModLoader = ModLoader::default(),
        /// Server type description
        pub server_type: Option<String> = None,
        /// Whether this is a development server
        pub development_mode: bool = false,
        /// Custom server properties
        pub properties: HashMap<String, String> = HashMap::new(),
        /// Server tags for organization
        pub tags: Vec<String> = vec![],
    }
}

config_struct! {
    /// Compatibility checking configuration for a server.
    pub struct CompatibilityConfig {
        /// Mods to ignore during compatibility checks
        pub ignore_mods: Vec<String> = vec![],
        /// Whether to auto-ignore client-only mods
        pub auto_ignore_client_only: bool = true,
        /// Whether to auto-ignore server-only mods
        pub auto_ignore_server_only: bool = true,
        /// Default compatibility profile to use
        pub default_profile: Option<String> = None,
        /// Custom compatibility rules
        pub custom_rules: Vec<String> = vec![],
    }
}

config_struct! {
    /// Individual server configuration.
    pub struct ServerConfig {
        /// Unique server identifier
        pub id: String = String::new(),
        /// Display name for the server
        pub name: String = String::new(),
        /// Server description
        pub description: Option<String> = None,
        /// Connection configuration
        pub connection: ConnectionType = ConnectionType::default(),
        /// Server-specific settings
        pub settings: ServerSettings = ServerSettings::default(),
        /// Compatibility checking configuration
        pub compatibility: CompatibilityConfig = CompatibilityConfig::default(),
        /// Last connection timestamp
        pub last_connected: Option<String> = None,
        /// Whether this server is enabled
        pub enabled: bool = true,
    }
}

config_struct! {
    /// Global servers configuration.
    pub struct ServersConfig {
        /// Version of the configuration format
        pub version: String = "1.0.0".to_string(),
        /// Map of server ID to server configuration
        pub servers: HashMap<String, ServerConfig> = HashMap::new(),
        /// Default server ID to use
        pub default_server: Option<String> = None,
        /// Global server settings
        pub global_settings: GlobalServerSettings = GlobalServerSettings::default(),
    }
}

config_struct! {
    /// Global settings that apply to all servers.
    pub struct GlobalServerSettings {
        /// Default connection timeout in seconds
        pub connection_timeout: u32 = 30,
        /// Default number of connection retries
        pub connection_retries: u32 = 3,
        /// Whether to verify SSL certificates for secure connections
        pub verify_ssl: bool = true,
        /// Default compatibility profile
        pub default_compatibility_profile: Option<String> = None,
        /// Global mod ignore list
        pub global_ignore_mods: Vec<String> = vec![],
    }
}

// Generate accessor methods for nested configs
config_accessors!(ServerConfig,
    settings: ServerSettings,
    compatibility: CompatibilityConfig
);

config_accessors!(ServersConfig,
    global_settings: GlobalServerSettings
);

impl ServerConfig {
    /// Creates a new server configuration with the given ID and name.
    pub fn new(id: String, name: String) -> Self {
        Self {
            id,
            name,
            ..Default::default()
        }
    }

    /// Validates the server configuration.
    pub fn validate(&self) -> Result<()> {
        if self.id.is_empty() {
            return Err(ConfigError::invalid_config(
                "id",
                "Server ID cannot be empty",
                None,
            ));
        }

        if self.name.is_empty() {
            return Err(ConfigError::invalid_config(
                "name",
                "Server name cannot be empty",
                None,
            ));
        }

        // Validate connection configuration
        match &self.connection {
            ConnectionType::Local(local) => {
                if local.path.is_empty() {
                    return Err(ConfigError::invalid_config(
                        "connection.path",
                        "Local path cannot be empty",
                        None,
                    ));
                }
            }
            ConnectionType::Ftp(ftp) => {
                if ftp.host.is_empty() {
                    return Err(ConfigError::invalid_config(
                        "connection.host",
                        "FTP host cannot be empty",
                        None,
                    ));
                }
                if ftp.username.is_empty() {
                    return Err(ConfigError::invalid_config(
                        "connection.username",
                        "FTP username cannot be empty",
                        None,
                    ));
                }
            }
            ConnectionType::Ssh(ssh) | ConnectionType::Sftp(ssh) => {
                if ssh.host.is_empty() {
                    return Err(ConfigError::invalid_config(
                        "connection.host",
                        "SSH host cannot be empty",
                        None,
                    ));
                }
                if ssh.username.is_empty() {
                    return Err(ConfigError::invalid_config(
                        "connection.username",
                        "SSH username cannot be empty",
                        None,
                    ));
                }
                if ssh.private_key_path.is_none() && ssh.password.is_none() {
                    return Err(ConfigError::invalid_config(
                        "connection.auth",
                        "Either private key path or password must be provided",
                        None,
                    ));
                }
            }
        }

        Ok(())
    }

    /// Gets the connection details as a PathBuf for local connections.
    pub fn get_local_path(&self) -> Option<PathBuf> {
        match &self.connection {
            ConnectionType::Local(local) => Some(PathBuf::from(&local.path)),
            _ => None,
        }
    }
}

impl ServersConfig {
    /// Loads configuration from directory.
    pub fn load(config_dir: &Path) -> Result<Self> {
        let config_file = config_dir.join("servers.toml");

        if config_file.exists() {
            let content = std::fs::read_to_string(&config_file).map_err(|e| {
                ConfigError::io_error(
                    "read servers config",
                    format!("Failed to read servers.toml: {}", e),
                    Some(e),
                )
            })?;

            toml::from_str(&content).map_err(|e| {
                ConfigError::serialization_error(
                    "TOML",
                    format!("Failed to parse servers.toml: {}", e),
                    Some(Box::new(e)),
                )
            })
        } else {
            // Create default config file
            let default_config = Self::default();
            default_config.save(config_dir)?;
            Ok(default_config)
        }
    }

    /// Saves configuration to directory.
    pub fn save(&self, config_dir: &Path) -> Result<()> {
        let config_file = config_dir.join("servers.toml");

        let content = toml::to_string_pretty(self).map_err(|e| {
            ConfigError::serialization_error(
                "TOML",
                format!("Failed to serialize servers config: {}", e),
                Some(Box::new(e)),
            )
        })?;

        std::fs::write(&config_file, content).map_err(|e| {
            ConfigError::io_error(
                "write servers config",
                format!("Failed to write servers.toml: {}", e),
                Some(e),
            )
        })?;

        Ok(())
    }

    /// Validates all server configurations.
    pub fn validate(&self) -> Result<()> {
        for (id, server) in &self.servers {
            if server.id != *id {
                return Err(ConfigError::invalid_config(
                    format!("servers.{}", id),
                    "Server ID in config doesn't match map key",
                    None,
                ));
            }
            server.validate()?;
        }
        Ok(())
    }
}
