use thiserror::Error;

/// Core error types for Minecraft server management
#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Connection failed: {message}")]
    ConnectionFailed { message: String },
    
    #[error("Authentication failed: {reason}")]
    AuthenticationFailed { reason: String },
    
    #[error("Server not found: {server_id}")]
    ServerNotFound { server_id: String },
    
    #[error("Invalid server configuration: {details}")]
    InvalidConfiguration { details: String },
    
    #[error("File operation failed: {operation} - {reason}")]
    FileOperationFailed { operation: String, reason: String },
    
    #[error("Server operation failed: {operation} - {reason}")]
    ServerOperationFailed { operation: String, reason: String },
    
    #[error("Network error: {message}")]
    NetworkError { message: String },
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error(transparent)]
    Logging(#[from] tosic_utils::logging::LoggingError)
}

/// Result type alias for operations that can fail with a [`CoreError`].
pub type Result<T> = std::result::Result<T, CoreError>;