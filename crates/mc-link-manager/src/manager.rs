use crate::{ManagerError, MinecraftStructure, Result, SyncAction, SyncPlan, SyncTarget};
use mc_link_compat::{CompatConfig, check_compatibility};
use mc_link_config::{CONFIG_MANAGER, ConnectionType, ServerConfig};
use mc_link_connector::{Connector, FtpConnector, LocalConnector};
use mc_link_core::{
    GlobalJarCache, ProgressCallback, ProgressReporter, ProgressStage, ProgressUpdate,
    ServerConnector, ServerStructureCache,
};
use std::{collections::HashMap, path::PathBuf};
use tracing::{debug, info};

/// High-level manager for Minecraft server instances.
///
/// Wraps a connector and provides operations for scanning, comparing,
/// and synchronizing Minecraft server files and configurations.
#[allow(dead_code)]
pub struct MinecraftManager<'a, C>
where
    C: ServerConnector,
{
    /// The underlying connector for server communication
    pub(crate) connector: C,
    server_config: Option<&'a ServerConfig>,
    /// Cached server structure (None = not scanned yet)
    structure: Option<MinecraftStructure>,
    /// Whether to enable parallel processing (default: true)
    pub(crate) parallel_enabled: bool,
    /// Global JAR cache for mod metadata
    pub(crate) jar_cache: Option<GlobalJarCache>,
    /// Server-specific structure cache
    pub(crate) structure_cache: Option<ServerStructureCache>,
    /// Progress reporter for long-running operations
    pub(crate) progress_reporter: Option<ProgressReporter>,
}

impl<'a> MinecraftManager<'a, Connector> {
    /// Creates a Minecraft manager from a server configuration.
    ///
    /// The instance will use the appropriate connector based on the
    /// `ConnectionType` specified in the configuration. This will introduce a slight overhead
    /// as the configuration must be parsed to determine during runtime.
    pub fn from_config(server_config: &'a ServerConfig) -> Self {
        let connector: Connector = match &server_config.connection {
            ConnectionType::Local(config) => LocalConnector::new(config).into(),
            ConnectionType::Ftp(config) => FtpConnector::new(config).into(),
            t => panic!("Unsupported connection type: `{}`.", t.type_name()),
        };

        debug!(
            connection_type = connector.connection_type(),
            "Created MinecraftManager from config"
        );

        let structure_cache =
            ServerStructureCache::load(server_config.id.clone(), &CONFIG_MANAGER.cache_dir())
                .unwrap_or_else(|_| ServerStructureCache::new(server_config.id.clone()));

        Self {
            connector,
            server_config: Some(server_config),
            structure: None,
            parallel_enabled: true,
            jar_cache: None,
            structure_cache: Some(structure_cache),
            progress_reporter: None,
        }
    }
}

impl<'a, C> MinecraftManager<'a, C>
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
            jar_cache: None,
            structure_cache: None,
            progress_reporter: None,
        }
    }

    /// Creates a manager with parallel processing disabled.
    pub fn new_sequential(connector: C) -> Self {
        Self {
            connector,
            server_config: None,
            structure: None,
            parallel_enabled: false,
            jar_cache: None,
            structure_cache: None,
            progress_reporter: None,
        }
    }

    /// Sets up caching for this manager using the global configuration.
    pub fn with_caching(mut self) -> Result<Self> {
        let config = &CONFIG_MANAGER.manager;

        if config.cache_enabled {
            // Set up global JAR cache
            let cache_dir = CONFIG_MANAGER.cache_dir();
            self.jar_cache = Some(GlobalJarCache::new(cache_dir, config.cache_max_size_mb)?);

            // Set up server-specific structure cache if we have server config
            if let Some(server_config) = self.server_config {
                self.structure_cache = Some(ServerStructureCache::new(server_config.id.clone()));
            }
        }

        Ok(self)
    }

    /// Sets a progress reporter for this manager.
    pub fn with_progress_reporter(mut self, reporter: ProgressReporter) -> Self {
        self.progress_reporter = Some(reporter);
        self
    }

    /// Reports progress if a reporter is set.
    pub(crate) fn report_progress(&self, update: ProgressUpdate) {
        if let Some(ref reporter) = self.progress_reporter {
            reporter(update);
        }
    }

    /// Connects to the server and scans its structure.
    ///
    /// This will scan the mods directory and other relevant folders,
    /// reading mod metadata in parallel for performance.
    /// Uses caching when available to improve performance.
    #[tracing::instrument(skip(self))]
    pub async fn scan(&mut self) -> Result<&MinecraftStructure> {
        use tracing::info;

        self.report_progress(ProgressUpdate::new(ProgressStage::Connecting, 0, 100));

        // Ensure we're connected
        if !self.connector.is_connected().await {
            debug!("Not connected, attempting to connect...");
            self.connector.connect().await?;
        }

        self.report_progress(ProgressUpdate::new(ProgressStage::CheckingCache, 10, 100));

        // Get server info to determine root structure
        let _server_info = self.connector.get_server_info().await?;

        // Check if we can use cached structure
        let config = &CONFIG_MANAGER.manager;
        let use_cache = if let Some(ref structure_cache) = self.structure_cache {
            config.cache_enabled && !structure_cache.is_expired(config.cache_ttl_hours)
        } else {
            false
        };

        if use_cache {
            // For now, we still perform a fresh scan since structure cache
            // doesn't store the full MinecraftStructure yet
            debug!("Structure cache available but performing fresh scan for completeness");
        }

        let mut structure = MinecraftStructure::new(PathBuf::from("."));

        self.report_progress(ProgressUpdate::new(ProgressStage::Listing, 20, 100));

        // Scan mods directory
        info!("Starting mod directory scan...");
        self.scan_mods_directory(&mut structure).await?;

        self.report_progress(ProgressUpdate::new(ProgressStage::Analyzing, 80, 100));

        info!("Found {} mods total", structure.mods.mods.len());

        // Log each mod found
        for (i, mod_info) in structure.mods.mods.iter().enumerate() {
            info!(
                index = i,
                mod_id = %mod_info.id,
                mod_name = %mod_info.name,
                version = ?mod_info.version,
                file_path = %mod_info.file_path.display(),
                side = ?mod_info.side,
                loader = ?mod_info.loader,
                "Scanned mod"
            );
        }

        self.report_progress(ProgressUpdate::new(ProgressStage::Listing, 90, 100));

        // Scan other directories
        structure.config.exists = self.check_directory_exists(&structure.config.path).await?;
        structure.resourcepacks.exists = self
            .check_directory_exists(&structure.resourcepacks.path)
            .await?;
        structure.shaderpacks.exists = self
            .check_directory_exists(&structure.shaderpacks.path)
            .await?;

        self.report_progress(ProgressUpdate::new(ProgressStage::UpdatingCache, 95, 100));

        // Update structure cache with new data
        if let Some(ref mut structure_cache) = self.structure_cache {
            let jar_hashes: Vec<String> = structure
                .mods
                .mods
                .iter()
                .filter_map(|mod_info| GlobalJarCache::compute_file_hash(&mod_info.file_path).ok())
                .collect();

            let mut directory_structure = HashMap::new();
            directory_structure.insert("mods".to_string(), structure.mods.exists);
            directory_structure.insert("config".to_string(), structure.config.exists);
            directory_structure.insert("resourcepacks".to_string(), structure.resourcepacks.exists);
            directory_structure.insert("shaderpacks".to_string(), structure.shaderpacks.exists);

            structure_cache.update(jar_hashes, directory_structure);

            // Save structure cache to disk
            let cache_dir = CONFIG_MANAGER.cache_dir();
            if let Err(e) = structure_cache.save(&cache_dir) {
                debug!("Failed to save structure cache: {}", e);
            }
        }

        // Save JAR cache if it exists
        if let Some(ref jar_cache) = self.jar_cache {
            if let Err(e) = jar_cache.save_cache_index() {
                debug!("Failed to save JAR cache index: {}", e);
            }
        }

        self.report_progress(ProgressUpdate::new(ProgressStage::Completed, 100, 100));

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
    pub async fn compare_with<'b, D>(
        &mut self,
        other: &mut MinecraftManager<'b, D>,
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

        // Perform compatibility check
        use tracing::info;
        info!(
            "Starting compatibility check: {} source mods vs {} target mods",
            self_structure.mods.mods.len(),
            other_structure.mods.mods.len()
        );

        let compat_result = check_compatibility(
            &self_structure.mods.mods,
            &other_structure.mods.mods,
            compat_config,
        )?;

        // Log detailed compatibility results
        info!(
            is_compatible = compat_result.is_compatible,
            missing_on_server = compat_result.missing_on_server.len(),
            missing_on_client = compat_result.missing_on_client.len(),
            version_mismatches = compat_result.version_mismatches.len(),
            ignored_mods = compat_result.ignored_mods.len(),
            "Compatibility check completed"
        );

        // Log missing mods details
        for mod_info in &compat_result.missing_on_server {
            info!(
                mod_name = %mod_info.name,
                version = ?mod_info.version,
                "Missing on server (will add)"
            );
        }

        for mod_info in &compat_result.missing_on_client {
            info!(
                mod_name = %mod_info.name,
                version = ?mod_info.version,
                "Missing on client (will remove from server)"
            );
        }

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
                mod_id: extra_mod.name.clone(), // Use the mod name as ID
                mod_info: extra_mod.clone(),
                target: SyncTarget::Server,
            });
        }

        // Handle version mismatches
        for version_mismatch in &compat_result.version_mismatches {
            // Find the source mod for the update
            if let Some(source_mod) = self_structure
                .mods
                .mods
                .iter()
                .find(|m| m.name == version_mismatch.mod_name)
            {
                if let Some(target_mod) = other_structure
                    .mods
                    .mods
                    .iter()
                    .find(|m| m.name == version_mismatch.mod_name)
                {
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
        if !self.connector.is_connected().await {
            self.connector.connect().await?;
        }

        for action in &plan.actions {
            match action {
                SyncAction::AddMod {
                    mod_info,
                    target: SyncTarget::Server,
                } => {
                    // Upload the mod file
                    let remote_path = PathBuf::from("mods")
                        .join(mod_info.file_path.file_name().unwrap_or_default());

                    self.connector
                        .upload_file(
                            &mod_info.file_path,
                            &remote_path,
                            None, // Progress callbacks not supported for individual file operations yet
                        )
                        .await
                        .map_err(|e| ManagerError::UpdateFailed {
                            action: format!("Add mod {}", mod_info.name),
                            reason: e.to_string(),
                        })?;
                }

                SyncAction::RemoveMod {
                    mod_info,
                    target: SyncTarget::Server,
                    ..
                } => {
                    // Delete the mod file
                    let remote_path = PathBuf::from("mods")
                        .join(mod_info.file_path.file_name().unwrap_or_default());

                    self.connector
                        .delete_file(&remote_path)
                        .await
                        .map_err(|e| ManagerError::UpdateFailed {
                            action: format!("Remove mod {}", mod_info.name),
                            reason: e.to_string(),
                        })?;
                }

                SyncAction::UpdateMod {
                    mod_id,
                    new_path,
                    current_path,
                    ..
                } => {
                    // Replace the mod file
                    let remote_path =
                        PathBuf::from("mods").join(current_path.file_name().unwrap_or_default());

                    // Delete old version
                    let _ = self.connector.delete_file(&remote_path).await;

                    // Upload new version
                    let new_remote_path =
                        PathBuf::from("mods").join(new_path.file_name().unwrap_or_default());

                    self.connector
                        .upload_file(
                            new_path,
                            &new_remote_path,
                            None, // Progress callbacks not supported for individual file operations yet
                        )
                        .await
                        .map_err(|e| ManagerError::UpdateFailed {
                            action: format!("Update mod {}", mod_id),
                            reason: e.to_string(),
                        })?;
                }

                SyncAction::KeepAsIs { .. } => {
                    // No action needed
                }

                // Handle client-side actions (not implemented for server connectors)
                SyncAction::AddMod {
                    target: SyncTarget::Client,
                    ..
                }
                | SyncAction::RemoveMod {
                    target: SyncTarget::Client,
                    ..
                } => {
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

    async fn check_directory_exists(&self, path: &std::path::Path) -> Result<bool> {
        let files = self.connector.list_files(&path.to_path_buf()).await;
        Ok(files.is_ok())
    }
}

impl<'a, C> std::fmt::Debug for MinecraftManager<'a, C>
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
