use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use suppaftp::AsyncFtpStream;
use suppaftp::list::File;
use tokio::io::AsyncReadExt;
use tokio::sync::Mutex;
use tracing::debug;
use mc_link_core::{CoreError, Result, ServerConnector, ServerInfo, ServerStatus, ProgressCallback};
use mc_link_config::FtpConnection;
use mc_link_core::traits::PathExt;

/// FTP connector for managing Minecraft servers over FTP protocol.
///
/// This connector provides file system access to manage servers via FTP,
/// supporting standard FTP operations without encryption.
pub struct FtpConnector {
    /// FTP server hostname
    host: String,
    /// FTP server port
    port: u16,
    /// Username for authentication
    username: String,
    /// Password for authentication
    password: String,
    /// Base path on the FTP server
    base_path: PathBuf,
    /// FTP connection stream wrapped in Arc<Mutex> for shared access
    ftp_stream: Arc<Mutex<Option<AsyncFtpStream>>>,
    /// Whether we're currently connected
    connected: Arc<Mutex<bool>>,
}

impl FtpConnector {
    /// Creates a new FTP connector with the specified connection details.
    ///
    /// # Arguments
    ///
    /// * `config` - FTP connection configuration
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use mc_link_config::FtpConnection;
    /// use mc_link_connector::FtpConnector;
    ///
    /// let config = FtpConnection {
    ///     host: "ftp.example.com".to_string(),
    ///     port: 21,
    ///     username: "username".to_string(),
    ///     password: Some("password".to_string()),
    ///     base_path: "/minecraft/server1".to_string(),
    ///     passive_mode: true,
    /// };
    /// let connector = FtpConnector::new(&config);
    /// ```
    pub fn new(config: &FtpConnection) -> Self {
        Self {
            host: config.host.clone(),
            port: config.port,
            username: config.username.clone(),
            password: config.password.clone().unwrap_or_default(),
            base_path: PathBuf::from(&config.base_path),
            ftp_stream: Arc::new(Mutex::new(None)),
            connected: Arc::new(Mutex::new(false)),
        }
    }
}

impl ServerConnector for FtpConnector {
    #[tracing::instrument(skip(self), fields(host = %self.host, port = %self.port, username = %self.username, base_path = %self.base_path.to_slash_lossy()))]
    fn connect(&mut self) -> impl std::future::Future<Output = Result<()>> + Send {
        let host = self.host.clone();
        let port = self.port;
        let username = self.username.clone();
        let password = self.password.clone();
        let base_path = self.base_path.clone();
        let ftp_stream = self.ftp_stream.clone();
        let connected = self.connected.clone();
        
        async move {
            // Connect to FTP server
            let mut ftp_stream_instance = AsyncFtpStream::connect(format!("{}:{}", host, port))
                .await
                .map_err(|e| CoreError::ConnectionFailed {
                    message: format!("Failed to connect to FTP server: {}", e),
                })?;
            
            // Login with credentials
            ftp_stream_instance.login(&username, &password)
                .await
                .map_err(|e| CoreError::AuthenticationFailed {
                    reason: format!("FTP login failed: {}", e),
                })?;
            
            // Set binary mode for file transfers
            ftp_stream_instance.transfer_type(suppaftp::types::FileType::Binary)
                .await
                .map_err(|e| CoreError::NetworkError {
                    message: format!("Failed to set binary mode: {}", e),
                })?;
            
            // Try to change to base directory to verify it exists
            let base_str = base_path.to_slash_lossy();
            ftp_stream_instance.cwd(&base_str)
                .await
                .map_err(|_e| CoreError::ServerNotFound {
                    server_id: base_str.to_string(),
                })?;
            
            *ftp_stream.lock().await = Some(ftp_stream_instance);
            *connected.lock().await = true;
            
            Ok(())
        }
    }

    #[tracing::instrument(skip(self), fields(host = %self.host))]
    fn disconnect(&mut self) -> impl std::future::Future<Output = Result<()>> + Send {
        let ftp_stream = self.ftp_stream.clone();
        let connected = self.connected.clone();
        async move {
            if let Some(mut stream) = ftp_stream.lock().await.take() {
                let _ = stream.quit().await;
            }
            *connected.lock().await = false;
            Ok(())
        }
    }
    
    fn is_connected(&self) -> impl std::future::Future<Output = bool> + Send {
        let connected = self.connected.clone();
        async move {
            *connected.lock().await
        }
    }
    
    fn get_server_info(&self) -> impl std::future::Future<Output = Result<ServerInfo>> + Send {
        let connected = self.connected.clone();
        async move {
            if !*connected.lock().await {
                return Err(CoreError::ConnectionFailed {
                    message: "Not connected to FTP server".to_string(),
                });
            }
            
            // We need to work around the borrow checker here
            // In practice, you'd want to restructure this or use Arc<Mutex<>>
            // For now, create a simplified response
            Ok(ServerInfo {
                version: None,
                properties: HashMap::new(),
                mods: Vec::new(),
                status: ServerStatus::Unknown,
                last_seen: None,
            })
        }
    }
    
    #[tracing::instrument(skip(self, progress), fields(local_path = %local_path.display(), remote_path = %remote_path.display()))]
    fn upload_file(
        &self,
        local_path: &PathBuf,
        remote_path: &PathBuf,
        progress: Option<ProgressCallback>,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
        let local_path = local_path.clone();
        let remote_path = remote_path.clone();
        let ftp_stream = self.ftp_stream.clone();
        let connected = self.connected.clone();
        
        async move {
            if !*connected.lock().await {
                return Err(CoreError::ConnectionFailed {
                    message: "Not connected to FTP server".to_string(),
                });
            }
            
            // Read local file
            let mut file = tokio::fs::File::open(&local_path).await
                .map_err(|e| CoreError::FileOperationFailed {
                    operation: "open local file".to_string(),
                    reason: e.to_string(),
                })?;
            
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer).await
                .map_err(|e| CoreError::FileOperationFailed {
                    operation: "read local file".to_string(),
                    reason: e.to_string(),
                })?;
            
            let total_size = buffer.len() as u64;
            
            // Upload file
            let mut ftp_stream = ftp_stream.lock().await;
            let stream = ftp_stream.as_mut().ok_or(CoreError::ConnectionFailed {
                message: "FTP stream is not initialized".to_string(),
            })?;
            
            // Create parent directories if needed
            if let Some(parent) = remote_path.parent() {
                let parent_str = parent.to_slash_lossy();
                if !parent_str.is_empty() && parent_str != "." {
                    let _ = stream.mkdir(&parent_str).await;
                }
            }
            
            // Upload the file using put_file with a byte slice
            let mut buffer_slice = buffer.as_slice();
            stream.put_file(&remote_path.to_slash_lossy(), &mut buffer_slice)
                .await
                .map_err(|e| CoreError::NetworkError {
                    message: format!("Failed to upload file: {}", e),
                })?;
            
            if let Some(callback) = progress {
                callback(total_size, total_size);
            }
            
            Ok(())
        }
    }
    
    #[tracing::instrument(skip(self, progress), fields(remote_path = %remote_path.display(), local_path = %local_path.display()))]
    fn download_file(
        &self,
        remote_path: &PathBuf,
        local_path: &PathBuf,
        progress: Option<ProgressCallback>,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
        let remote_path = remote_path.clone();
        let local_path = local_path.clone();
        let ftp_stream = self.ftp_stream.clone();
        let connected = self.connected.clone();
        
        async move {
            if !*connected.lock().await {
                return Err(CoreError::ConnectionFailed {
                    message: "Not connected to FTP server".to_string(),
                });
            }
            
            // Create parent directories for local file if needed
            if let Some(parent) = local_path.parent() {
                tokio::fs::create_dir_all(parent).await
                    .map_err(|e| CoreError::FileOperationFailed {
                        operation: "create local directories".to_string(),
                        reason: e.to_string(),
                    })?;
            }
            
            // Download file
            let mut ftp_stream = ftp_stream.lock().await;
            let stream = ftp_stream.as_mut().ok_or(CoreError::ConnectionFailed {
                message: "FTP stream is not initialized".to_string(),
            })?;

            // Use retr method with proper callback to download to a buffer
            let buffer = stream.retr(&remote_path.to_slash_lossy(), |mut data_stream| {
                Box::pin(async move {
                    use futures_lite::io::AsyncReadExt;
                    let mut buffer = Vec::new();
                    match data_stream.read_to_end(&mut buffer).await {
                        Ok(_) => Ok((buffer, data_stream)),
                        Err(e) => Err(suppaftp::FtpError::ConnectionError(e))
                    }
                })
            })
                .await
                .map_err(|e| CoreError::NetworkError {
                    message: format!("Failed to download file: {}", e),
                })?;
            
            let total_size = buffer.len() as u64;
            
            // Write to local file
            tokio::fs::write(&local_path, &buffer).await
                .map_err(|e| CoreError::FileOperationFailed {
                    operation: "write local file".to_string(),
                    reason: e.to_string(),
                })?;
            
            if let Some(callback) = progress {
                callback(total_size, total_size);
            }
            
            Ok(())
        }
    }
    
    fn list_files(&self, remote_path: &PathBuf) -> impl std::future::Future<Output = Result<Vec<PathBuf>>> + Send {
        let remote_path = remote_path.clone();
        let ftp_stream = self.ftp_stream.clone();
        let connected = self.connected.clone();
        
        async move {
            if !*connected.lock().await {
                return Err(CoreError::ConnectionFailed {
                    message: "Not connected to FTP server".to_string(),
                });
            }

            let mut ftp_stream = ftp_stream.lock().await;
            let stream = ftp_stream.as_mut().ok_or(CoreError::ConnectionFailed {
                message: "FTP stream is not initialized".to_string(),
            })?;

            let entries = stream.list(Some(&remote_path.to_slash_lossy()))
                .await
                .map_err(|e| CoreError::NetworkError {
                    message: format!("Failed to list files: {}", e),
                })?;
            
            let files: Vec<PathBuf> = entries.iter()
                .filter_map(|entry| {
                    if let Ok(file) = File::try_from(entry.as_str()) {
                        if !file.is_directory() {
                            debug!("Listing file: {}", file.name());
                            // Return the full path relative to base_path, including the directory being listed
                            Some(remote_path.join(file.name()))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect();
            
            Ok(files)
        }
    }
    
    fn delete_file(&self, remote_path: &PathBuf) -> impl std::future::Future<Output = Result<()>> + Send {
        let remote_path = remote_path.clone();
        let ftp_stream = self.ftp_stream.clone();
        let connected = self.connected.clone();
        
        async move {
            if !*connected.lock().await {
                return Err(CoreError::ConnectionFailed {
                    message: "Not connected to FTP server".to_string(),
                });
            }
            
            let mut ftp_stream = ftp_stream.lock().await;
            let stream = ftp_stream.as_mut().ok_or(CoreError::ConnectionFailed {
                message: "FTP stream is not initialized".to_string(),
            })?;
            
            // Try to delete as file first, then as directory
            let path_str = remote_path.to_slash_lossy();
            match stream.rm(&path_str).await {
                Ok(_) => Ok(()),
                Err(_) => {
                    // Try as directory
                    stream.rmdir(&path_str).await
                        .map_err(|e| CoreError::FileOperationFailed {
                            operation: "delete file/directory".to_string(),
                            reason: format!("Failed to delete: {}", e),
                        })
                }
            }
        }
    }
    
    fn create_directory(&self, remote_path: &PathBuf) -> impl std::future::Future<Output = Result<()>> + Send {
        let remote_path = remote_path.clone();
        let ftp_stream = self.ftp_stream.clone();
        let connected = self.connected.clone();
        
        async move {
            if !*connected.lock().await {
                return Err(CoreError::ConnectionFailed {
                    message: "Not connected to FTP server".to_string(),
                });
            }
            
            let mut ftp_stream = ftp_stream.lock().await;
            let stream = ftp_stream.as_mut().ok_or(CoreError::ConnectionFailed {
                message: "FTP stream is not initialized".to_string(),
            })?;
            
            let path_str = remote_path.to_slash_lossy();
            stream.mkdir(&path_str).await
                .map_err(|e| CoreError::FileOperationFailed {
                    operation: "create directory".to_string(),
                    reason: format!("Failed to create directory: {}", e),
                })?;
            
            Ok(())
        }
    }
    
    fn execute_command(&self, _command: &str) -> impl std::future::Future<Output = Result<String>> + Send {
        async move {
            Err(CoreError::ServerOperationFailed {
                operation: "execute command".to_string(),
                reason: "Command execution not supported for FTP connector".to_string(),
            })
        }
    }
}