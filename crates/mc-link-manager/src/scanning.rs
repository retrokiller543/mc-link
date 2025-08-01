use crate::{ManagerError, MinecraftStructure, Result};
use futures::future::join_all;
use mc_link_compat::extract_jar_info;
use mc_link_config::CONFIG_MANAGER;
use mc_link_core::{GlobalJarCache, ModInfo, ProgressStage, ProgressUpdate, ServerConnector};
use std::{path::PathBuf, sync::{atomic::{AtomicU64, Ordering}, Arc}};
use tracing::{debug, trace, warn};

/// Scanning functionality for discovering and analyzing mods
impl<'a, C> super::MinecraftManager<'a, C>
where
    C: ServerConnector + Send + Sync + 'static,
{
    /// Scans the mods directory using the appropriate strategy (parallel/sequential)
    #[tracing::instrument(skip(self, structure), fields(parallel = self.parallel_enabled))]
    pub async fn scan_mods_directory(&mut self, structure: &mut MinecraftStructure) -> Result<()> {
        let mods_path = &structure.mods.path;

        self.report_progress(ProgressUpdate::with_message(
            ProgressStage::Listing,
            0,
            100,
            "Listing mod files".to_string(),
        ));

        // Check if mods directory exists
        let mod_files = self.connector.list_files(&mods_path).await?;
        structure.mods.exists = !mod_files.is_empty();

        if !structure.mods.exists {
            return Ok(());
        }

        // Filter for JAR files
        let jar_files: Vec<_> = mod_files
            .into_iter()
            .filter(|path| {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case("jar"))
                    .unwrap_or(false)
            })
            .collect();

        self.report_progress(ProgressUpdate::with_message(
            ProgressStage::Downloading,
            10,
            100,
            format!("Processing {} JAR files", jar_files.len()),
        ));

        if self.parallel_enabled {
            self.scan_mods_parallel(&jar_files, structure).await?;
        } else {
            self.scan_mods_sequential(&jar_files, structure).await?;
        }

        Ok(())
    }

    /// Scans mods in parallel for better performance
    #[tracing::instrument(skip(self, structure), fields(jar_count = jar_files.len()))]
    async fn scan_mods_parallel(
        &mut self,
        jar_files: &[PathBuf],
        structure: &mut MinecraftStructure,
    ) -> Result<()> {
        let temp_dir = self.create_temp_directory("mc-link-scan-parallel").await?;
        let downloaded_files = self.download_jars_parallel(jar_files, &temp_dir).await?;
        let mod_infos = self.analyze_jars_parallel(downloaded_files).await?;
        self.add_mods_to_structure(structure, mod_infos).await?;
        self.cleanup_temp_directory(&temp_dir).await;
        Ok(())
    }

    /// Creates a temporary directory for mod scanning
    #[inline]
    async fn create_temp_directory(&self, name: &str) -> Result<PathBuf> {
        let temp_dir = std::env::temp_dir().join(name);
        tokio::fs::create_dir_all(&temp_dir).await.map_err(|e| {
            ManagerError::FileOperationFailed {
                operation: "create temp directory".to_string(),
                reason: e.to_string(),
            }
        })?;
        Ok(temp_dir)
    }

    /// Downloads JAR files in parallel for analysis with progress updates
    #[tracing::instrument(skip(self), fields(jar_count = jar_files.len()))]
    async fn download_jars_parallel(
        &mut self,
        jar_files: &[PathBuf],
        temp_dir: &PathBuf,
    ) -> Result<Vec<(PathBuf, PathBuf)>> {
        let total_files = jar_files.len() as u64;
        let completed_count = Arc::new(AtomicU64::new(0));
        let reporter = Arc::new(&self.progress_reporter);
        
        // Report initial download progress
        self.report_progress(ProgressUpdate::with_message(
            ProgressStage::Downloading,
            0,
            total_files,
            "Starting file downloads".to_string(),
        ));

        let download_futures: Vec<_> = jar_files
            .iter()
            .map(|jar_file| {
                let local_jar_path = temp_dir.join(jar_file.file_name().unwrap_or_default());
                let jar_file = jar_file.clone();
                let connector = &self.connector;
                let completed_count = Arc::clone(&completed_count);
                let progress_reporter = Arc::clone(&reporter);

                async move {
                    let result = match connector
                        .download_file(&jar_file, &local_jar_path, None)
                        .await
                    {
                        Ok(_) => {
                            trace!(file_path = %jar_file.display(), "Downloaded JAR");
                            Some((jar_file, local_jar_path))
                        }
                        Err(e) => {
                            warn!(
                                file_path = %jar_file.display(),
                                error = %e,
                                "Failed to download JAR file"
                            );
                            None
                        }
                    };
                    
                    // Update progress atomically
                    let current = completed_count.fetch_add(1, Ordering::Relaxed) + 1;

                    if let Some(ref reporter) = **progress_reporter {
                        reporter(ProgressUpdate::with_message(
                            ProgressStage::Downloading,
                            current,
                            total_files,
                            format!("Downloaded {} of {} files", current, total_files),
                        ));
                    }
                    
                    result
                }
            })
            .collect();

        let download_results = join_all(download_futures).await;
        let successful_count = download_results.iter().filter(|r| r.is_some()).count();
        let downloaded_files: Vec<_> = download_results
            .into_iter()
            .filter_map(|result| result)
            .collect();

        trace!(
            total_files = jar_files.len(),
            successful_downloads = successful_count,
            "Download results"
        );

        if successful_count == 0 && !jar_files.is_empty() {
            warn!(
                total_files = jar_files.len(),
                "Failed to download any JAR files"
            );
        }

        Ok(downloaded_files)
    }

    /// Analyzes JAR files in parallel to extract mod metadata
    #[tracing::instrument(skip(self, downloaded_files), fields(file_count = downloaded_files.len()))]
    async fn analyze_jars_parallel(
        &mut self,
        downloaded_files: Vec<(PathBuf, PathBuf)>,
    ) -> Result<Vec<ModInfo>> {
        let total_files = downloaded_files.len();
        let mut mod_infos = Vec::with_capacity(total_files);

        // Report analysis progress
        self.report_progress(ProgressUpdate::with_message(
            ProgressStage::Analyzing,
            0,
            total_files as u64,
            "Starting JAR analysis".to_string(),
        ));

        // Process each file with progress updates
        for (i, (remote_path, local_path)) in downloaded_files.into_iter().enumerate() {
            let mod_info = self.analyze_single_jar(&remote_path, &local_path).await;
            let _ = tokio::fs::remove_file(&local_path).await;
            mod_infos.push(mod_info);

            // Update progress
            self.report_progress(ProgressUpdate::with_message(
                ProgressStage::Analyzing,
                (i + 1) as u64,
                total_files as u64,
                format!("Analyzed {} of {} files", i + 1, total_files),
            ));
        }

        trace!(mod_count = mod_infos.len(), "JAR analysis completed");
        for (i, mod_info) in mod_infos.iter().enumerate() {
            trace!(
                index = i,
                mod_id = %mod_info.id,
                mod_name = %mod_info.name,
                file_path = %mod_info.file_path.display(),
                "Analyzed mod"
            );
        }

        Ok(mod_infos)
    }

    /// Analyzes a single JAR file to extract mod metadata, using cache if available
    async fn analyze_single_jar(&mut self, remote_path: &PathBuf, local_path: &PathBuf) -> ModInfo {
        let config = &CONFIG_MANAGER.manager;

        // Try to use cache if enabled
        if config.cache_enabled {
            if let Ok(hash) = GlobalJarCache::compute_file_hash(local_path) {
                if let Some(ref mut jar_cache) = self.jar_cache {
                    if let Some(cached_mod_info) = jar_cache.get(&hash, config.cache_ttl_hours) {
                        // Update the file path to the current remote path
                        let mut mod_info = cached_mod_info;
                        mod_info.file_path = remote_path.clone();
                        return mod_info;
                    }
                }
            }
        }

        // Cache miss or cache disabled - analyze the JAR
        let jar_info = extract_jar_info(local_path);
        let mod_name = remote_path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_string();

        let mod_info = match jar_info {
            Ok(mut compat_mod_info) => {
                compat_mod_info.file_path = remote_path.clone();
                compat_mod_info
            }
            Err(_) => ModInfo {
                id: mod_name.clone(),
                name: mod_name,
                version: None,
                file_path: remote_path.clone(),
                enabled: true,
                side: mc_link_core::ModSide::Unknown,
                loader: mc_link_core::ModLoader::Unknown,
                raw_metadata: std::collections::HashMap::new(),
            },
        };

        // Store in cache if enabled
        if config.cache_enabled {
            if let Ok(hash) = GlobalJarCache::compute_file_hash(local_path) {
                if let Some(ref mut jar_cache) = self.jar_cache {
                    let file_size = std::fs::metadata(local_path).map(|m| m.len()).unwrap_or(0);
                    let filename = local_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown.jar")
                        .to_string();

                    let _ = jar_cache.put(hash, filename, file_size, mod_info.clone());
                }
            }
        }

        mod_info
    }

    /// Adds analyzed mods to the structure
    #[inline]
    async fn add_mods_to_structure(
        &self,
        structure: &mut MinecraftStructure,
        mod_infos: Vec<ModInfo>,
    ) -> Result<()> {
        let initial_count = structure.mods.mods.len();
        structure.mods.mods.extend(mod_infos);
        debug!(
            added_mods = structure.mods.mods.len() - initial_count,
            total_mods = structure.mods.mods.len(),
            "Added mods to structure"
        );
        Ok(())
    }

    /// Cleans up temporary directory
    #[inline]
    async fn cleanup_temp_directory(&self, temp_dir: &PathBuf) {
        let _ = tokio::fs::remove_dir_all(temp_dir).await;
    }

    /// Scans mods sequentially (fallback for when parallel scanning fails)
    async fn scan_mods_sequential(
        &mut self,
        jar_files: &[PathBuf],
        structure: &mut MinecraftStructure,
    ) -> Result<()> {
        let temp_dir = std::env::temp_dir().join("mc-link-scan");
        tokio::fs::create_dir_all(&temp_dir).await.map_err(|e| {
            ManagerError::FileOperationFailed {
                operation: "create temp directory".to_string(),
                reason: e.to_string(),
            }
        })?;

        for jar_file in jar_files {
            // Download JAR file to temp directory
            let local_jar_path = temp_dir.join(jar_file.file_name().unwrap_or_default());

            match self
                .connector
                .download_file(jar_file, &local_jar_path, None)
                .await
            {
                Ok(_) => {
                    // Extract JAR info from downloaded file
                    match extract_jar_info(&local_jar_path) {
                        Ok(mut compat_mod_info) => {
                            // Update file path to remote path (jar extraction uses local temp path)
                            compat_mod_info.file_path = jar_file.clone();
                            structure.mods.mods.push(compat_mod_info);
                        }
                        Err(_) => {
                            // Fallback to filename-based info if JAR analysis fails
                            let mod_name = jar_file
                                .file_stem()
                                .and_then(|name| name.to_str())
                                .unwrap_or("unknown")
                                .to_string();

                            let mod_info = ModInfo {
                                id: mod_name.clone(),
                                name: mod_name,
                                version: None,
                                file_path: jar_file.clone(),
                                enabled: true,
                                side: mc_link_core::ModSide::Unknown,
                                loader: mc_link_core::ModLoader::Unknown,
                                raw_metadata: std::collections::HashMap::new(),
                            };
                            structure.mods.mods.push(mod_info);
                        }
                    }

                    // Clean up downloaded file
                    let _ = tokio::fs::remove_file(&local_jar_path).await;
                }
                Err(_) => {
                    // If download fails, create basic mod info from filename
                    let mod_name = jar_file
                        .file_stem()
                        .and_then(|name| name.to_str())
                        .unwrap_or("unknown")
                        .to_string();

                    let mod_info = ModInfo {
                        id: mod_name.clone(),
                        name: mod_name,
                        version: None,
                        file_path: jar_file.clone(),
                        enabled: true,
                        side: mc_link_core::ModSide::Unknown,
                        loader: mc_link_core::ModLoader::Unknown,
                        raw_metadata: std::collections::HashMap::new(),
                    };
                    structure.mods.mods.push(mod_info);
                }
            }
        }

        // Clean up temp directory
        let _ = tokio::fs::remove_dir(&temp_dir).await;

        Ok(())
    }
}
