use thiserror::Error;

/// Errors that can occur during Minecraft server management operations.
#[derive(Error, Debug)]
pub enum ManagerError {
    /// Core error from underlying operations
    #[error(transparent)]
    Core(#[from] mc_link_core::CoreError),
    
    /// Compatibility checking error
    #[error(transparent)]
    Compat(#[from] mc_link_compat::CompatError),
    
    /// Failed to execute update action
    #[error("Update action failed: {action} - {reason}")]
    UpdateFailed { action: String, reason: String },
    
    /// Server structure validation error
    #[error("Invalid server structure: {reason}")]
    InvalidStructure { reason: String },
    
    /// Parallel processing error
    #[error("Parallel operation failed: {operation} - {reason}")]
    ParallelError { operation: String, reason: String },

    #[error("Failed to perform file operation {operation} - {reason}")]
    FileOperationFailed { operation: String, reason: String },
}

/// Result type for manager operations.
pub type Result<T> = std::result::Result<T, ManagerError>;