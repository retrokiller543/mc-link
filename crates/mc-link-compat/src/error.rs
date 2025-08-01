use thiserror::Error;

/// Errors that can occur during mod compatibility checking.
#[derive(Error, Debug)]
pub enum CompatError {
    /// Failed to read or parse a JAR file
    #[error("JAR file error: {file} - {reason}")]
    JarError { file: String, reason: String },

    /// Failed to parse mod metadata
    #[error("Metadata parsing error: {mod_name} - {reason}")]
    MetadataError { mod_name: String, reason: String },

    /// Core error passthrough
    #[error(transparent)]
    Core(#[from] mc_link_core::CoreError),

    /// IO error occurred
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON parsing error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// TOML parsing error
    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),

    /// ZIP file error
    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),
}

/// Result type for compatibility operations.
pub type Result<T> = std::result::Result<T, CompatError>;
