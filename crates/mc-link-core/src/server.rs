use std::collections::HashMap;
use std::path::PathBuf;
use std::future::Future;
use serde::{Deserialize, Serialize};
use crate::Result;

/// Ensures a connector is connected before performing operations.
///
/// # Examples
///
/// ```ignore
/// use mc_link_core::ensure_connected;
/// 
/// fn some_operation(connector: &impl ServerConnector) -> Result<()> {
///     ensure_connected!(connector);
///     // ... perform operation
///     Ok(())
/// }
/// ```
///
/// # Errors
///
/// Returns [`CoreError::ConnectionFailed`] if the connector is not connected.
#[macro_export]
macro_rules! ensure_connected {
    ($conn:expr) => {
        if !$conn.is_connected().await {
            return Err(CoreError::ConnectionFailed {
                message: "Not connected to server".to_string(),
            });
        }
    };
    ($conn:expr, $msg:expr) => {
        if !$conn.is_connected().await {
            return Err(CoreError::ConnectionFailed {
                message: $msg.to_string(),
            });
        }
    };
}

/// Represents the current status of a Minecraft server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerStatus {
    /// Server is running and accepting connections
    Online,
    /// Server is not running
    Offline,
    /// Server is in the process of starting up
    Starting,
    /// Server is in the process of shutting down
    Stopping,
    /// Server status cannot be determined
    Unknown,
}

/// Contains comprehensive information about a Minecraft server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    /// Server version (e.g., "1.20.1", "Paper 1.20.1")
    pub version: Option<String>,
    /// Server properties from server.properties file
    pub properties: HashMap<String, String>,
    /// List of installed mods
    pub mods: Vec<ModInfo>,
    /// Current server status
    pub status: ServerStatus,
    /// Timestamp of last successful connection (Unix timestamp)
    pub last_seen: Option<u64>,
}

/// Represents the side a mod runs on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModSide {
    /// Mod runs on client only
    Client,
    /// Mod runs on server only
    Server,
    /// Mod runs on both client and server
    Both,
    /// Side is unknown or unspecified
    Unknown,
}

/// Supported mod loaders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModLoader {
    /// NeoForge mod loader
    NeoForge,
    /// Fabric mod loader
    Fabric,
    /// Legacy Forge mod loader
    Forge,
    /// Unknown or unsupported mod loader
    Unknown,
}

/// Information about a single mod installed on the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModInfo {
    /// Unique mod identifier (used for comparison)
    pub id: String,
    /// Display name of the mod
    pub name: String,
    /// Version string of the mod, if available
    pub version: Option<String>,
    /// Path to the mod file
    pub file_path: PathBuf,
    /// Whether the mod is currently enabled
    pub enabled: bool,
    /// Which side(s) the mod runs on
    pub side: ModSide,
    /// Mod loader type
    pub loader: ModLoader,
    /// Raw metadata for advanced processing
    pub raw_metadata: std::collections::HashMap<String, serde_json::Value>,
}

/// Callback function for tracking file transfer progress.
///
/// Called periodically during file operations with (bytes_transferred, total_bytes).
pub type ProgressCallback = Box<dyn Fn(u64, u64) + Send + Sync>;

/// Core trait for managing connections to Minecraft servers.
///
/// This trait defines the interface for connecting to and managing Minecraft servers
/// across different platforms and connection methods (local filesystem, FTP, SSH, etc.).
pub trait ServerConnector: Send + Sync {
    /// Connect to the server with the given connection details
    fn connect(&mut self) -> impl Future<Output = Result<()>> + Send;
    
    /// Disconnect from the server
    fn disconnect(&mut self) -> impl Future<Output = Result<()>> + Send;
    
    /// Check if currently connected to the server
    fn is_connected(&self) -> impl Future<Output = bool> + Send;
    
    /// Get server information (version, properties, mods, etc.)
    fn get_server_info(&self) -> impl Future<Output = Result<ServerInfo>> + Send;
    
    /// Upload a file to the server
    fn upload_file(
        &self,
        local_path: &PathBuf,
        remote_path: &PathBuf,
        progress: Option<ProgressCallback>,
    ) -> impl Future<Output = Result<()>> + Send;
    
    /// Download a file from the server
    fn download_file(
        &self,
        remote_path: &PathBuf,
        local_path: &PathBuf,
        progress: Option<ProgressCallback>,
    ) -> impl Future<Output = Result<()>> + Send;
    
    /// List files in a remote directory
    fn list_files(&self, remote_path: &PathBuf) -> impl Future<Output = Result<Vec<PathBuf>>> + Send;
    
    /// Delete a file on the server
    fn delete_file(&self, remote_path: &PathBuf) -> impl Future<Output = Result<()>> + Send;
    
    /// Create a directory on the server
    fn create_directory(&self, remote_path: &PathBuf) -> impl Future<Output = Result<()>> + Send;
    
    /// Execute a server command (if supported)
    fn execute_command(&self, command: &str) -> impl Future<Output = Result<String>> + Send;
}