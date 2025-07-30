use std::path::PathBuf;
use tracing::debug;
use mc_link_core::{ServerConnector, ModInfo, ProgressCallback};
use mc_link_compat::{CompatConfig, check_compatibility};
use mc_link_config::{ConnectionType, ServerConfig};
use mc_link_connector::{Connector, FtpConnector, LocalConnector};
use crate::{ManagerError, Result, MinecraftStructure, SyncPlan, SyncAction, SyncTarget};

/// High-level manager for Minecraft server instances.
///
/// Wraps a connector and provides operations for scanning, comparing,
/// and synchronizing Minecraft server files and configurations.
#[allow(dead_code)]
pub struct MinecraftManager<C> 
where 
    C: ServerConnector,
{
    /// The underlying connector for server communication
    connector: C,
    server_config: Option<&'static ServerConfig>,
    /// Cached server structure (None = not scanned yet)
    structure: Option<MinecraftStructure>,
    /// Whether to enable parallel processing (default: true)
    parallel_enabled: bool,
}

impl MinecraftManager<Connector> {
    /// Creates a Minecraft manager from a server configuration.
    ///
    /// The instance will use the appropriate connector based on the
    /// `ConnectionType` specified in the configuration. This will introduce a slight overhead
    /// as the configuration must be parsed to determine during runtime.
    pub fn from_config(server_config: &'static ServerConfig) -> Self {
        let connector: Connector = match &server_config.connection {
            ConnectionType::Local(config) => LocalConnector::new(config).into(),
            ConnectionType::Ftp(config) => FtpConnector::new(config).into(),
            t => panic!("Unsupported connection type: `{}`.", t.type_name()),
        };
        
        debug!(connection_type = connector.connection_type(), "Created MinecraftManager from config");

        Self {
            connector,
            server_config: Some(server_config),
            structure: None,
            parallel_enabled: true,
        }
    }
}

impl<C> MinecraftManager<C>
where
    C: ServerConnector + Send + Sync + 'static,
{
    /// Creates a new Minecraft manager with the given connector.
    ///
    /// # Arguments
    ///
    /// * `connector` - The server connector to use for operations
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use mc_link_manager::MinecraftManager;
    /// use mc_link_connector::LocalConnector;
    /// use std::path::PathBuf;
    ///
    /// let connector = LocalConnector::new(PathBuf::from("/opt/minecraft/server"));
    /// let manager = MinecraftManager::new(connector);
    /// ```
    pub fn new(connector: C) -> Self {
        Self {
            connector,
            server_config: None,
            structure: None,
            parallel_enabled: true,
        }
    }
    
    /// Creates a manager with parallel processing disabled.
    pub fn new_sequential(connector: C) -> Self {
        Self {
            connector,
            server_config: None,
            structure: None,
            parallel_enabled: false,
        }
    }
    
    /// Connects to the server and scans its structure.
    ///
    /// This will scan the mods directory and other relevant folders,
    /// reading mod metadata in parallel for performance.
    #[tracing::instrument]
    pub async fn scan(&mut self) -> Result<&MinecraftStructure> {
        // Ensure we're connected
        if !self.connector.is_connected() {
            debug!("Not connected, attempting to connect...");
            self.connector.connect().await?;
        }
        
        // Get server info to determine root structure
        let _server_info = self.connector.get_server_info().await?;
        
        let mut structure = MinecraftStructure::new(PathBuf::from("."));
        
        // Scan mods directory
        self.scan_mods_directory(&mut structure).await?;
        
        // Scan other directories
        structure.config.exists = self.check_directory_exists(&structure.config.path).await?;
        structure.resourcepacks.exists = self.check_directory_exists(&structure.resourcepacks.path).await?;
        structure.shaderpacks.exists = self.check_directory_exists(&structure.shaderpacks.path).await?;
        
        self.structure = Some(structure);
        Ok(self.structure.as_ref().unwrap())
    }
    
    /// Compares this manager's instance with another and returns a sync plan.
    ///
    /// The returned plan contains actions needed to make the `other` manager's
    /// instance match this manager's instance.
    ///
    /// # Arguments
    ///
    /// * `other` - The other Minecraft manager to compare with
    /// * `compat_config` - Configuration for compatibility checking
    ///
    /// # Returns
    ///
    /// A `SyncPlan` containing actions to synchronize `other` to match `self`.
    pub async fn compare_with<D>(
        &mut self,
        other: &mut MinecraftManager<D>,
        compat_config: &CompatConfig,
    ) -> Result<SyncPlan>
    where
        D: ServerConnector + Send + Sync + 'static,
    {
        // Ensure both managers have scanned their structures
        let self_structure = match self.structure {
            Some(ref s) => s,
            None => {
                self.scan().await?;
                self.structure.as_ref().unwrap()
            }
        };
        
        let other_structure = match other.structure { 
            Some(ref s) => s,
            None => {
                other.scan().await?;
                other.structure.as_ref().unwrap()
            }
        };
        
        let other_structure = other.structure.as_ref()
            .ok_or_else(|| ManagerError::InvalidStructure {
                reason: "Other structure not scanned. Call scan() on other manager first.".to_string(),
            })?;
        
        // Perform compatibility check
        let compat_result = check_compatibility(
            &self_structure.mods.mods,
            &other_structure.mods.mods,
            compat_config,
        )?;
        
        // Build sync plan based on compatibility results
        let mut plan = SyncPlan::new();
        plan.will_be_compatible = compat_result.is_compatible;
        
        // Handle missing mods on target (other)
        for missing_mod in &compat_result.missing_on_server {
            plan.add_action(SyncAction::AddMod {
                mod_info: missing_mod.clone(),
                target: SyncTarget::Server,
            });
        }
        
        // Handle missing mods on source (self) - these should be removed from target
        for extra_mod in &compat_result.missing_on_client {
            plan.add_action(SyncAction::RemoveMod {
                mod_id: extract_mod_id(extra_mod),
                mod_info: extra_mod.clone(),
                target: SyncTarget::Server,
            });
        }
        
        // Handle version mismatches
        for version_mismatch in &compat_result.version_mismatches {
            // Find the source mod for the update
            if let Some(source_mod) = self_structure.mods.mods.iter()
                .find(|m| extract_mod_id(m) == version_mismatch.mod_id) {
                
                if let Some(target_mod) = other_structure.mods.mods.iter()
                    .find(|m| extract_mod_id(m) == version_mismatch.mod_id) {
                    
                    plan.add_action(SyncAction::UpdateMod {
                        mod_id: version_mismatch.mod_id.clone(),
                        from_version: version_mismatch.server_version.clone(),
                        to_version: version_mismatch.client_version.clone(),
                        current_path: target_mod.file_path.clone(),
                        new_path: source_mod.file_path.clone(),
                    });
                }
            }
        }
        
        // Handle ignored mods
        for ignored_mod_id in &compat_result.ignored_mods {
            plan.add_action(SyncAction::KeepAsIs {
                mod_id: ignored_mod_id.clone(),
                reason: "Mod ignored by compatibility rules".to_string(),
            });
        }
        
        Ok(plan)
    }
    
    /// Executes a sync plan, performing the actual file operations.
    ///
    /// # Arguments
    ///
    /// * `plan` - The sync plan to execute
    /// * `progress` - Optional progress callback for file operations
    pub async fn execute_sync_plan(
        &mut self,
        plan: &SyncPlan,
        _progress: Option<ProgressCallback>,
    ) -> Result<()> {
        if !self.connector.is_connected() {
            self.connector.connect().await?;
        }
        
        for action in &plan.actions {
            match action {
                SyncAction::AddMod { mod_info, target: SyncTarget::Server } => {
                    // Upload the mod file
                    let remote_path = PathBuf::from("mods")
                        .join(mod_info.file_path.file_name().unwrap_or_default());
                    
                    self.connector.upload_file(
                        &mod_info.file_path,
                        &remote_path,
                        None, // TODO: Forward progress callback properly
                    ).await.map_err(|e| ManagerError::UpdateFailed {
                        action: format!("Add mod {}", mod_info.name),
                        reason: e.to_string(),
                    })?;
                }
                
                SyncAction::RemoveMod { mod_info, target: SyncTarget::Server, .. } => {
                    // Delete the mod file
                    let remote_path = PathBuf::from("mods")
                        .join(mod_info.file_path.file_name().unwrap_or_default());
                    
                    self.connector.delete_file(&remote_path).await
                        .map_err(|e| ManagerError::UpdateFailed {
                            action: format!("Remove mod {}", mod_info.name),
                            reason: e.to_string(),
                        })?;
                }
                
                SyncAction::UpdateMod { mod_id, new_path, current_path, .. } => {
                    // Replace the mod file
                    let remote_path = PathBuf::from("mods")
                        .join(current_path.file_name().unwrap_or_default());
                    
                    // Delete old version
                    let _ = self.connector.delete_file(&remote_path).await;
                    
                    // Upload new version
                    let new_remote_path = PathBuf::from("mods")
                        .join(new_path.file_name().unwrap_or_default());
                    
                    self.connector.upload_file(
                        new_path,
                        &new_remote_path,
                        None, // TODO: Forward progress callback properly
                    ).await.map_err(|e| ManagerError::UpdateFailed {
                        action: format!("Update mod {}", mod_id),
                        reason: e.to_string(),
                    })?;
                }
                
                SyncAction::KeepAsIs { .. } => {
                    // No action needed
                }
                
                // Handle client-side actions (not implemented for server connectors)
                SyncAction::AddMod { target: SyncTarget::Client, .. } |
                SyncAction::RemoveMod { target: SyncTarget::Client, .. } => {
                    return Err(ManagerError::UpdateFailed {
                        action: "Client-side action".to_string(),
                        reason: "Cannot perform client actions with server connector".to_string(),
                    });
                }
            }
        }
        
        // Invalidate cached structure to force rescan
        self.structure = None;
        
        Ok(())
    }
    
    /// Returns a reference to the cached structure, if available.
    pub fn structure(&self) -> Option<&MinecraftStructure> {
        self.structure.as_ref()
    }
    
    /// Forces a fresh scan, ignoring any cached structure.
    pub async fn refresh(&mut self) -> Result<&MinecraftStructure> {
        self.structure = None;
        self.scan().await
    }
    
    #[tracing::instrument]
    async fn scan_mods_directory(&mut self, structure: &mut MinecraftStructure) -> Result<()> {
        let mods_path = &structure.mods.path;
        
        // Check if mods directory exists
        let mod_files = self.connector.list_files(&mods_path).await?;
        structure.mods.exists = !mod_files.is_empty();
        
        if !structure.mods.exists {
            return Ok(());
        }
        
        // Filter for JAR files
        let jar_files: Vec<_> = mod_files.into_iter()
            .filter(|path| {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case("jar"))
                    .unwrap_or(false)
            })
            .collect();
        
        if self.parallel_enabled {
            // Process JAR files in parallel
            self.scan_mods_parallel(&jar_files, structure).await?;
        } else {
            // Process JAR files sequentially
            self.scan_mods_sequential(&jar_files, structure).await?;
        }
        
        Ok(())
    }
    
    async fn scan_mods_parallel(
        &mut self,
        jar_files: &[PathBuf],
        structure: &mut MinecraftStructure,
    ) -> Result<()> {
        // For parallel processing, we'd need to download files first
        // This is a simplified version - in practice, you'd want to:
        // 1. Download all JAR files to a temp directory
        // 2. Process them in parallel
        // 3. Clean up temp files
        
        // For now, fall back to sequential processing
        self.scan_mods_sequential(jar_files, structure).await
    }
    
    async fn scan_mods_sequential(
        &mut self,
        jar_files: &[PathBuf],
        structure: &mut MinecraftStructure,
    ) -> Result<()> {
        for jar_file in jar_files {
            // Create basic mod info from filename
            let mod_name = jar_file.file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("unknown")
                .to_string();
            
            let mod_info = ModInfo {
                name: mod_name,
                version: None, // TODO: Extract from JAR metadata when downloaded
                file_path: jar_file.clone(),
                enabled: true,
            };
            
            structure.mods.mods.push(mod_info);
        }
        
        Ok(())
    }
    
    async fn check_directory_exists(&self, path: &std::path::Path) -> Result<bool> {
        let files = self.connector.list_files(&path.to_path_buf()).await;
        Ok(files.is_ok())
    }
}

/// Extracts a mod ID from ModInfo for comparison purposes.
fn extract_mod_id(mod_info: &ModInfo) -> String {
    // Simple extraction - in practice, this should use the same logic
    // as the compatibility checker
    mod_info.file_path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(&mod_info.name)
        .to_string()
}

impl<C> std::fmt::Debug for MinecraftManager<C>
where
    C: ServerConnector,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MinecraftManager")
            .field("structure", &self.structure)
            .field("parallel_enabled", &self.parallel_enabled)
            .finish_non_exhaustive()
    }
}
