use crate::{ConfigError, config_enum, config_struct};
use std::path::Path;

config_enum! {
    pub enum LogLevel {
        /// Debug level logging
        Debug,
        /// Info level logging
        Info,
        /// Warning level logging
        Warn,
        /// Error level logging
        Error,
        /// Critical level logging
        Critical,
    }
    default = Info
}

config_enum! {
    pub enum LogFileNameFormat {
        Date,
        Timestamp,
        DateTime,
        None,
    }
    default = DateTime
}

config_struct! {
    pub struct ManagerConfig {
        /// log level for the manager
        pub log_level: LogLevel = LogLevel::default(),
        pub log_file: LogFileNameFormat = LogFileNameFormat::default(),
        /// whether to log to stdout in addition to file
        pub log_to_stdout: bool = false,
    }
}

impl ManagerConfig {
    /// Loads configuration from directory.
    pub fn load(config_dir: &Path) -> crate::Result<Self> {
        let config_file = config_dir.join("manager.toml");

        if config_file.exists() {
            let content = std::fs::read_to_string(&config_file).map_err(|e| {
                ConfigError::io_error(
                    "read manager config",
                    format!("Failed to read manager.toml: {}", e),
                    Some(e),
                )
            })?;

            toml::from_str(&content).map_err(|e| {
                ConfigError::serialization_error(
                    "TOML",
                    format!("Failed to parse manager.toml: {}", e),
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
    pub fn save(&self, config_dir: &Path) -> crate::Result<()> {
        let config_file = config_dir.join("manager.toml");

        let content = toml::to_string_pretty(self).map_err(|e| {
            ConfigError::serialization_error(
                "TOML",
                format!("Failed to serialize managers config: {}", e),
                Some(Box::new(e)),
            )
        })?;

        std::fs::write(&config_file, content).map_err(|e| {
            ConfigError::io_error(
                "write managers config",
                format!("Failed to write manager.toml: {}", e),
                Some(e),
            )
        })?;

        Ok(())
    }

    /// Validates all server configurations.
    pub fn validate(&self) -> crate::Result<()> {
        Ok(())
    }
}
