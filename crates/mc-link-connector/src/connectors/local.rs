use std::collections::HashMap;
use std::path::PathBuf;
use mc_link_core::{CoreError, Result, ServerConnector, ServerInfo, ServerStatus, ModInfo, ProgressCallback, ensure_connected};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use mc_link_config::LocalConnection;
use std::future::Future;

/// Connector for managing Minecraft servers on the local filesystem.
///
/// This connector allows direct file system access to manage servers running
/// on the same machine as the management tool.
pub struct LocalConnector {
    server_path: PathBuf,
    connected: bool,
}

impl LocalConnector {
    /// Creates a new local connector for the specified server directory.
    ///
    /// # Arguments
    ///
    /// * `config` - Local connection configuration
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use mc_link_config::LocalConnection;
    /// use mc_link_connector::LocalConnector;
    ///
    /// let config = LocalConnection { path: "/opt/minecraft/server1".to_string() };
    /// let connector = LocalConnector::new(&config);
    /// ```
    pub fn new(config: &LocalConnection) -> Self {
        Self {
            server_path: PathBuf::from(&config.path),
            connected: false,
        }
    }
    
    /// Get the server.properties file path
    fn properties_file(&self) -> PathBuf {
        self.server_path.join("server.properties")
    }
    
    /// Get the mods directory path
    fn mods_dir(&self) -> PathBuf {
        self.server_path.join("mods")
    }
    
    /// Parse server.properties file
    async fn parse_properties(&self) -> Result<HashMap<String, String>> {
        let properties_path = self.properties_file();
        
        if !properties_path.exists() {
            return Ok(HashMap::new());
        }
        
        let content = fs::read_to_string(&properties_path).await
            .map_err(|e| CoreError::FileOperationFailed {
                operation: "read server.properties".to_string(),
                reason: e.to_string(),
            })?;
        
        let mut properties = HashMap::new();
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            
            if let Some((key, value)) = line.split_once('=') {
                properties.insert(key.trim().to_string(), value.trim().to_string());
            }
        }
        
        Ok(properties)
    }
    
    /// Scan mods directory for mod files
    async fn scan_mods(&self) -> Result<Vec<ModInfo>> {
        let mods_path = self.mods_dir();
        
        if !mods_path.exists() {
            return Ok(Vec::new());
        }
        
        let mut mods = Vec::new();
        let mut entries = fs::read_dir(&mods_path).await
            .map_err(|e| CoreError::FileOperationFailed {
                operation: "read mods directory".to_string(),
                reason: e.to_string(),
            })?;
        
        while let Some(entry) = entries.next_entry().await
            .map_err(|e| CoreError::FileOperationFailed {
                operation: "read mods directory entry".to_string(),
                reason: e.to_string(),
            })? {
            
            let path = entry.path();
            if let Some(extension) = path.extension() {
                if extension == "jar" {
                    let name = path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    
                    mods.push(ModInfo {
                        name,
                        version: None,
                        file_path: path,
                        enabled: true,
                    });
                }
            }
        }
        
        Ok(mods)
    }
}

impl ServerConnector for LocalConnector {
    fn connect(&mut self) -> impl Future<Output = Result<()>> + Send {
        async move {
            if !self.server_path.exists() {
                return Err(CoreError::ServerNotFound {
                    server_id: self.server_path.display().to_string(),
                });
            }
            
            if !self.server_path.is_dir() {
                return Err(CoreError::InvalidConfiguration {
                    details: "Server path is not a directory".to_string(),
                });
            }
            
            self.connected = true;
            Ok(())
        }
    }
    
    fn disconnect(&mut self) -> impl Future<Output = Result<()>> + Send {
        async move {
            self.connected = false;
            Ok(())
        }
    }
    
    fn is_connected(&self) -> bool {
        self.connected
    }
    
    fn get_server_info(&self) -> impl Future<Output = Result<ServerInfo>> + Send {
        async move {
            ensure_connected!(self);
            
            let properties = self.parse_properties().await?;
            let mods = self.scan_mods().await?;
            
            Ok(ServerInfo {
                version: None,
                properties,
                mods,
                status: ServerStatus::Unknown,
                last_seen: None,
            })
        }
    }
    
    fn upload_file(
        &self,
        local_path: &PathBuf,
        remote_path: &PathBuf,
        progress: Option<ProgressCallback>,
    ) -> impl Future<Output = Result<()>> + Send {
        let local_path = local_path.clone();
        let remote_path = remote_path.clone();
        let server_path = self.server_path.clone();
        
        async move {
            ensure_connected!(self);
            
            let full_remote_path = server_path.join(&remote_path);
            
            if let Some(parent) = full_remote_path.parent() {
                fs::create_dir_all(parent).await
                    .map_err(|e| CoreError::FileOperationFailed {
                        operation: "create directories".to_string(),
                        reason: e.to_string(),
                    })?;
            }
            
            let mut source = fs::File::open(&local_path).await
                .map_err(|e| CoreError::FileOperationFailed {
                    operation: "open source file".to_string(),
                    reason: e.to_string(),
                })?;
            
            let mut dest = fs::File::create(&full_remote_path).await
                .map_err(|e| CoreError::FileOperationFailed {
                    operation: "create destination file".to_string(),
                    reason: e.to_string(),
                })?;
            
            let total_size = source.metadata().await
                .map_err(|e| CoreError::FileOperationFailed {
                    operation: "get file metadata".to_string(),
                    reason: e.to_string(),
                })?.len();
            
            let mut buffer = vec![0u8; 8192];
            let mut bytes_copied = 0u64;
            
            loop {
                let bytes_read = source.read(&mut buffer).await
                    .map_err(|e| CoreError::FileOperationFailed {
                        operation: "read from source".to_string(),
                        reason: e.to_string(),
                    })?;
                
                if bytes_read == 0 {
                    break;
                }
                
                dest.write_all(&buffer[..bytes_read]).await
                    .map_err(|e| CoreError::FileOperationFailed {
                        operation: "write to destination".to_string(),
                        reason: e.to_string(),
                    })?;
                
                bytes_copied += bytes_read as u64;
                
                if let Some(ref callback) = progress {
                    callback(bytes_copied, total_size);
                }
            }
            
            Ok(())
        }
    }
    
    fn download_file(
        &self,
        remote_path: &PathBuf,
        local_path: &PathBuf,
        progress: Option<ProgressCallback>,
    ) -> impl Future<Output = Result<()>> + Send {
        let remote_path = remote_path.clone();
        let local_path = local_path.clone();
        let server_path = self.server_path.clone();
        
        async move {
            ensure_connected!(self);
            
            let full_remote_path = server_path.join(&remote_path);
            
            if let Some(parent) = local_path.parent() {
                fs::create_dir_all(parent).await
                    .map_err(|e| CoreError::FileOperationFailed {
                        operation: "create local directories".to_string(),
                        reason: e.to_string(),
                    })?;
            }
            
            let mut source = fs::File::open(&full_remote_path).await
                .map_err(|e| CoreError::FileOperationFailed {
                    operation: "open remote file".to_string(),
                    reason: e.to_string(),
                })?;
            
            let mut dest = fs::File::create(&local_path).await
                .map_err(|e| CoreError::FileOperationFailed {
                    operation: "create local file".to_string(),
                    reason: e.to_string(),
                })?;
            
            let total_size = source.metadata().await
                .map_err(|e| CoreError::FileOperationFailed {
                    operation: "get remote file metadata".to_string(),
                    reason: e.to_string(),
                })?.len();
            
            let mut buffer = vec![0u8; 8192];
            let mut bytes_copied = 0u64;
            
            loop {
                let bytes_read = source.read(&mut buffer).await
                    .map_err(|e| CoreError::FileOperationFailed {
                        operation: "read from remote".to_string(),
                        reason: e.to_string(),
                    })?;
                
                if bytes_read == 0 {
                    break;
                }
                
                dest.write_all(&buffer[..bytes_read]).await
                    .map_err(|e| CoreError::FileOperationFailed {
                        operation: "write to local".to_string(),
                        reason: e.to_string(),
                    })?;
                
                bytes_copied += bytes_read as u64;
                
                if let Some(ref callback) = progress {
                    callback(bytes_copied, total_size);
                }
            }
            
            Ok(())
        }
    }
    
    fn list_files(&self, remote_path: &PathBuf) -> impl Future<Output = Result<Vec<PathBuf>>> + Send {
        let remote_path = remote_path.clone();
        let server_path = self.server_path.clone();
        
        async move {
            ensure_connected!(self);
            
            let full_path = server_path.join(&remote_path);
            
            if !full_path.exists() {
                return Ok(Vec::new());
            }
            
            let mut files = Vec::new();
            let mut entries = fs::read_dir(&full_path).await
                .map_err(|e| CoreError::FileOperationFailed {
                    operation: "read directory".to_string(),
                    reason: e.to_string(),
                })?;
            
            while let Some(entry) = entries.next_entry().await
                .map_err(|e| CoreError::FileOperationFailed {
                    operation: "read directory entry".to_string(),
                    reason: e.to_string(),
                })? {
                
                let path = entry.path();
                if let Ok(relative_path) = path.strip_prefix(&server_path) {
                    files.push(relative_path.to_path_buf());
                }
            }
            
            Ok(files)
        }
    }
    
    fn delete_file(&self, remote_path: &PathBuf) -> impl Future<Output = Result<()>> + Send {
        let remote_path = remote_path.clone();
        let server_path = self.server_path.clone();
        
        async move {
            ensure_connected!(self);
            
            let full_path = server_path.join(&remote_path);
            
            if full_path.is_dir() {
                fs::remove_dir_all(&full_path).await
            } else {
                fs::remove_file(&full_path).await
            }.map_err(|e| CoreError::FileOperationFailed {
                operation: "delete file".to_string(),
                reason: e.to_string(),
            })?;
            
            Ok(())
        }
    }
    
    fn create_directory(&self, remote_path: &PathBuf) -> impl Future<Output = Result<()>> + Send {
        let remote_path = remote_path.clone();
        let server_path = self.server_path.clone();
        
        async move {
            ensure_connected!(self);
            
            let full_path = server_path.join(&remote_path);
            
            fs::create_dir_all(&full_path).await
                .map_err(|e| CoreError::FileOperationFailed {
                    operation: "create directory".to_string(),
                    reason: e.to_string(),
                })?;
            
            Ok(())
        }
    }
    
    fn execute_command(&self, _command: &str) -> impl Future<Output = Result<String>> + Send {
        async move {
            Err(CoreError::ServerOperationFailed {
                operation: "execute command".to_string(),
                reason: "Command execution not supported for local connector".to_string(),
            })
        }
    }
}
