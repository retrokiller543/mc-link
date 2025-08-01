//! Configuration error types

use thiserror::Error;

/// Configuration system errors for Minecraft server management.
#[derive(Error, Debug)]
pub enum ConfigError {
    /// IO error occurred during config operations
    #[error("IO error: {operation} - {reason}")]
    IoError {
        operation: String,
        reason: String,
        #[source]
        cause: Option<std::io::Error>,
    },

    /// Configuration serialization/deserialization failed
    #[error("Serialization error: {format} - {reason}")]
    SerializationError {
        format: String,
        reason: String,
        #[source]
        cause: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Configuration validation failed
    #[error("Invalid configuration: {field} - {reason}")]
    InvalidConfig {
        field: String,
        reason: String,
        #[source]
        cause: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Server configuration not found
    #[error("Server not found: {server_id}")]
    ServerNotFound {
        server_id: String,
        #[source]
        cause: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Connection configuration is invalid
    #[error("Invalid connection config: {connection_type} - {reason}")]
    InvalidConnectionConfig {
        connection_type: String,
        reason: String,
        #[source]
        cause: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Profile or template not found
    #[error("Profile not found: {profile_name}")]
    ProfileNotFound {
        profile_name: String,
        #[source]
        cause: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Standard IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// TOML serialization error
    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),
}

impl ConfigError {
    /// Creates an IO error with optional cause.
    pub fn io_error(
        operation: impl Into<String>,
        reason: impl Into<String>,
        cause: Option<std::io::Error>,
    ) -> Self {
        Self::IoError {
            operation: operation.into(),
            reason: reason.into(),
            cause,
        }
    }

    /// Creates a serialization error with optional cause.
    pub fn serialization_error(
        format: impl Into<String>,
        reason: impl Into<String>,
        cause: Option<Box<dyn std::error::Error + Send + Sync>>,
    ) -> Self {
        Self::SerializationError {
            format: format.into(),
            reason: reason.into(),
            cause,
        }
    }

    /// Creates an invalid config error with optional cause.
    pub fn invalid_config(
        field: impl Into<String>,
        reason: impl Into<String>,
        cause: Option<Box<dyn std::error::Error + Send + Sync>>,
    ) -> Self {
        Self::InvalidConfig {
            field: field.into(),
            reason: reason.into(),
            cause,
        }
    }
}

/// Result type for configuration operations.
pub type Result<T, E = ConfigError> = std::result::Result<T, E>;
