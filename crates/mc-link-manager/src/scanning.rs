use std::path::PathBuf;
use futures::future::join_all;
use tracing::{debug, warn, trace};
use mc_link_core::{ServerConnector, ModInfo};
use mc_link_compat::extract_jar_info;
use crate::{ManagerError, Result, MinecraftStructure};

/// Scanning functionality for discovering and analyzing mods
impl<C> super::MinecraftManager<C>
where
    C: ServerConnector + Send + Sync + 'static,
{
    /// Scans the mods directory using the appropriate strategy (parallel/sequential)
    #[tracing::instrument(skip(self, structure), fields(parallel = self.parallel_enabled))]
    pub async fn scan_mods_directory(&mut self, structure: &mut MinecraftStructure) -> Result<()> {
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
        tokio::fs::create_dir_all(&temp_dir).await
            .map_err(|e| ManagerError::FileOperationFailed {
                operation: "create temp directory".to_string(),
                reason: e.to_string(),
            })?;
        Ok(temp_dir)
    }

    /// Downloads JAR files in parallel for analysis
    #[tracing::instrument(skip(self), fields(jar_count = jar_files.len()))]
    async fn download_jars_parallel(
        &self,
        jar_files: &[PathBuf],
        temp_dir: &PathBuf,
    ) -> Result<Vec<(PathBuf, PathBuf)>> {
        let download_futures: Vec<_> = jar_files.iter().map(|jar_file| {
            let local_jar_path = temp_dir.join(jar_file.file_name().unwrap_or_default());
            let jar_file = jar_file.clone();
            let connector = &self.connector;
            
            async move {
                match connector.download_file(&jar_file, &local_jar_path, None).await {
                    Ok(_) => {
                        trace!(file_path = %jar_file.display(), "Downloaded JAR");
                        Some((jar_file, local_jar_path))
                    },
                    Err(e) => {
                        warn!(
                            file_path = %jar_file.display(),
                            error = %e,
                            "Failed to download JAR file"
                        );
                        None
                    },
                }
            }
        }).collect();

        let download_results = join_all(download_futures).await;
        let successful_count = download_results.iter().filter(|r| r.is_some()).count();
        let downloaded_files: Vec<_> = download_results.into_iter()
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
        &self,
        downloaded_files: Vec<(PathBuf, PathBuf)>,
    ) -> Result<Vec<ModInfo>> {
        let analysis_futures: Vec<_> = downloaded_files.into_iter().map(|(remote_path, local_path)| {
            async move {
                let mod_info = self.analyze_single_jar(&remote_path, &local_path).await;
                let _ = tokio::fs::remove_file(&local_path).await;
                mod_info
            }
        }).collect();

        let mod_infos = join_all(analysis_futures).await;
        
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

    /// Analyzes a single JAR file to extract mod metadata
    #[inline]
    async fn analyze_single_jar(&self, remote_path: &PathBuf, local_path: &PathBuf) -> ModInfo {
        let jar_info = extract_jar_info(local_path);
        let mod_name = remote_path.file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_string();

        match jar_info {
            Ok(mut compat_mod_info) => {
                compat_mod_info.file_path = remote_path.clone();
                compat_mod_info
            },
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
        }
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
        tokio::fs::create_dir_all(&temp_dir).await
            .map_err(|e| ManagerError::FileOperationFailed {
                operation: "create temp directory".to_string(),
                reason: e.to_string(),
            })?;

        for jar_file in jar_files {
            // Download JAR file to temp directory
            let local_jar_path = temp_dir.join(jar_file.file_name().unwrap_or_default());
            
            match self.connector.download_file(jar_file, &local_jar_path, None).await {
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
                            let mod_name = jar_file.file_stem()
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
                    let mod_name = jar_file.file_stem()
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