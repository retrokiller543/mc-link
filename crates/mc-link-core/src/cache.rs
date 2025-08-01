//! Caching infrastructure for Minecraft server management.

use crate::{CoreError, ModInfo, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Represents cached JAR file metadata with content-based addressing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedJarInfo {
    /// SHA-256 hash of the JAR file contents
    pub hash: String,
    /// Original filename (for reference)
    pub filename: String,
    /// File size in bytes
    pub size: u64,
    /// Extracted mod information
    pub mod_info: ModInfo,
    /// When this entry was cached (seconds since Unix epoch)
    pub cached_at: u64,
    /// Last time this entry was accessed (for LRU eviction)
    pub last_accessed: u64,
}

impl CachedJarInfo {
    /// Creates a new cached JAR info entry.
    pub fn new(hash: String, filename: String, size: u64, mod_info: ModInfo) -> Self {
        let now = current_timestamp();
        Self {
            hash,
            filename,
            size,
            mod_info,
            cached_at: now,
            last_accessed: now,
        }
    }

    /// Updates the last accessed timestamp.
    pub fn touch(&mut self) {
        self.last_accessed = current_timestamp();
    }

    /// Returns the age of this cache entry in seconds.
    pub fn age_seconds(&self) -> u64 {
        current_timestamp().saturating_sub(self.cached_at)
    }

    /// Returns whether this cache entry has expired based on TTL.
    pub fn is_expired(&self, ttl_hours: u32) -> bool {
        let ttl_seconds = ttl_hours as u64 * 3600;
        self.age_seconds() > ttl_seconds
    }
}

/// Global JAR cache for storing mod metadata indexed by content hash.
#[derive(Debug)]
pub struct GlobalJarCache {
    cache_dir: PathBuf,
    entries: HashMap<String, CachedJarInfo>,
    max_size_bytes: u64,
    current_size_bytes: u64,
}

impl GlobalJarCache {
    /// Creates a new global JAR cache.
    pub fn new(cache_dir: PathBuf, max_size_mb: u32) -> Result<Self> {
        let jar_cache_dir = cache_dir.join("jars");
        std::fs::create_dir_all(&jar_cache_dir).map_err(|e| CoreError::FileOperationFailed {
            operation: "create jar cache directory".to_string(),
            reason: format!("Failed to create cache directory: {}", e),
        })?;

        let mut cache = Self {
            cache_dir: jar_cache_dir,
            entries: HashMap::new(),
            max_size_bytes: max_size_mb as u64 * 1024 * 1024,
            current_size_bytes: 0,
        };

        cache.load_cache_index()?;
        Ok(cache)
    }

    /// Computes SHA-256 hash of file contents.
    pub fn compute_file_hash(file_path: &Path) -> Result<String> {
        let contents = std::fs::read(file_path).map_err(|e| CoreError::FileOperationFailed {
            operation: "read file for hashing".to_string(),
            reason: format!("Failed to read file {}: {}", file_path.display(), e),
        })?;

        let mut hasher = Sha256::new();
        hasher.update(&contents);
        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Checks if a JAR with the given hash is cached and valid.
    /// Returns a clone of the ModInfo to avoid borrowing issues.
    pub fn get(&mut self, hash: &str, ttl_hours: u32) -> Option<ModInfo> {
        // First check if entry exists and is valid
        let is_expired = self
            .entries
            .get(hash)
            .map(|entry| entry.is_expired(ttl_hours))
            .unwrap_or(true);

        if !is_expired {
            // Entry is valid, touch it and return a clone
            if let Some(entry) = self.entries.get_mut(hash) {
                entry.touch();
                return Some(entry.mod_info.clone());
            }
        } else if self.entries.contains_key(hash) {
            // Entry has expired, remove it
            if let Some(entry) = self.entries.remove(hash) {
                self.current_size_bytes = self.current_size_bytes.saturating_sub(entry.size);
                let _ = self.remove_cache_file(hash);
            }
        }

        None
    }

    /// Adds a new JAR to the cache.
    pub fn put(
        &mut self,
        hash: String,
        filename: String,
        size: u64,
        mod_info: ModInfo,
    ) -> Result<()> {
        // Check if we need to evict entries to make space
        while self.current_size_bytes + size > self.max_size_bytes && !self.entries.is_empty() {
            self.evict_lru()?;
        }

        let cached_info = CachedJarInfo::new(hash.clone(), filename, size, mod_info);

        // Save to disk
        self.save_cache_entry(&hash, &cached_info)?;

        // Update in-memory state
        self.entries.insert(hash, cached_info);
        self.current_size_bytes += size;

        Ok(())
    }

    /// Removes the least recently used entry.
    fn evict_lru(&mut self) -> Result<()> {
        let oldest_hash = self
            .entries
            .iter()
            .min_by_key(|(_, entry)| entry.last_accessed)
            .map(|(hash, _)| hash.clone());

        if let Some(hash) = oldest_hash {
            if let Some(entry) = self.entries.remove(&hash) {
                self.current_size_bytes = self.current_size_bytes.saturating_sub(entry.size);
                self.remove_cache_file(&hash)?;
            }
        }

        Ok(())
    }

    /// Clears all cached entries.
    pub fn clear(&mut self) -> Result<()> {
        for hash in self.entries.keys() {
            let _ = self.remove_cache_file(hash);
        }
        self.entries.clear();
        self.current_size_bytes = 0;
        Ok(())
    }

    /// Returns cache statistics.
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            entry_count: self.entries.len(),
            total_size_bytes: self.current_size_bytes,
            max_size_bytes: self.max_size_bytes,
        }
    }

    /// Validates and cleans up expired entries.
    pub fn cleanup(&mut self, ttl_hours: u32) -> Result<u32> {
        let mut removed_count = 0;
        let expired_hashes: Vec<String> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.is_expired(ttl_hours))
            .map(|(hash, _)| hash.clone())
            .collect();

        for hash in expired_hashes {
            if let Some(entry) = self.entries.remove(&hash) {
                self.current_size_bytes = self.current_size_bytes.saturating_sub(entry.size);
                let _ = self.remove_cache_file(&hash);
                removed_count += 1;
            }
        }

        Ok(removed_count)
    }

    /// Loads the cache index from disk.
    fn load_cache_index(&mut self) -> Result<()> {
        let index_file = self.cache_dir.join("index.json");
        if !index_file.exists() {
            return Ok(());
        }

        let content =
            std::fs::read_to_string(&index_file).map_err(|e| CoreError::FileOperationFailed {
                operation: "read cache index".to_string(),
                reason: format!("Failed to read cache index: {}", e),
            })?;

        let entries: HashMap<String, CachedJarInfo> =
            serde_json::from_str(&content).map_err(|e| CoreError::FileOperationFailed {
                operation: "parse cache index".to_string(),
                reason: format!("Failed to parse cache index: {}", e),
            })?;

        self.current_size_bytes = entries.values().map(|e| e.size).sum();
        self.entries = entries;

        Ok(())
    }

    /// Saves the cache index to disk.
    pub fn save_cache_index(&self) -> Result<()> {
        let index_file = self.cache_dir.join("index.json");
        let content = serde_json::to_string_pretty(&self.entries).map_err(|e| {
            CoreError::FileOperationFailed {
                operation: "serialize cache index".to_string(),
                reason: format!("Failed to serialize cache index: {}", e),
            }
        })?;

        std::fs::write(&index_file, content).map_err(|e| CoreError::FileOperationFailed {
            operation: "write cache index".to_string(),
            reason: format!("Failed to write cache index: {}", e),
        })?;

        Ok(())
    }

    /// Saves a single cache entry to disk.
    fn save_cache_entry(&self, hash: &str, entry: &CachedJarInfo) -> Result<()> {
        let entry_file = self.cache_dir.join(format!("{}.json", hash));
        let content =
            serde_json::to_string_pretty(entry).map_err(|e| CoreError::FileOperationFailed {
                operation: "serialize cache entry".to_string(),
                reason: format!("Failed to serialize cache entry: {}", e),
            })?;

        std::fs::write(&entry_file, content).map_err(|e| CoreError::FileOperationFailed {
            operation: "write cache entry".to_string(),
            reason: format!("Failed to write cache entry: {}", e),
        })?;

        Ok(())
    }

    /// Removes a cache file from disk.
    fn remove_cache_file(&self, hash: &str) -> Result<()> {
        let entry_file = self.cache_dir.join(format!("{}.json", hash));
        if entry_file.exists() {
            std::fs::remove_file(&entry_file).map_err(|e| CoreError::FileOperationFailed {
                operation: "remove cache file".to_string(),
                reason: format!("Failed to remove cache file: {}", e),
            })?;
        }
        Ok(())
    }
}

impl Drop for GlobalJarCache {
    fn drop(&mut self) {
        let _ = self.save_cache_index();
    }
}

/// Cache statistics for monitoring and management.
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub entry_count: usize,
    pub total_size_bytes: u64,
    pub max_size_bytes: u64,
}

impl CacheStats {
    /// Returns the cache usage as a percentage (0.0-1.0).
    pub fn usage_percentage(&self) -> f64 {
        if self.max_size_bytes == 0 {
            0.0
        } else {
            self.total_size_bytes as f64 / self.max_size_bytes as f64
        }
    }

    /// Returns the total size in a human-readable format.
    pub fn total_size_formatted(&self) -> String {
        format_bytes(self.total_size_bytes)
    }

    /// Returns the max size in a human-readable format.
    pub fn max_size_formatted(&self) -> String {
        format_bytes(self.max_size_bytes)
    }
}

/// Server-specific structure cache for storing scan results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerStructureCache {
    /// Server ID this cache belongs to
    pub server_id: String,
    /// List of JAR file hashes found on this server
    pub jar_hashes: Vec<String>,
    /// Directory structure metadata
    pub directory_structure: HashMap<String, bool>,
    /// When this structure was last scanned
    pub last_scanned: u64,
    /// Remote file modification times for validation
    pub file_mtimes: HashMap<String, u64>,
}

impl ServerStructureCache {
    /// Creates a new server structure cache.
    pub fn new(server_id: String) -> Self {
        Self {
            server_id,
            jar_hashes: Vec::new(),
            directory_structure: HashMap::new(),
            last_scanned: current_timestamp(),
            file_mtimes: HashMap::new(),
        }
    }

    /// Checks if the cache is expired based on TTL.
    pub fn is_expired(&self, ttl_hours: u32) -> bool {
        let ttl_seconds = ttl_hours as u64 * 3600;
        let age = current_timestamp().saturating_sub(self.last_scanned);
        age > ttl_seconds
    }

    /// Updates the cache with new scan results.
    pub fn update(&mut self, jar_hashes: Vec<String>, directory_structure: HashMap<String, bool>) {
        self.jar_hashes = jar_hashes;
        self.directory_structure = directory_structure;
        self.last_scanned = current_timestamp();
    }

    /// Saves the structure cache to disk.
    pub fn save(&self, cache_dir: &Path) -> Result<()> {
        std::fs::create_dir_all(cache_dir)?;
        let cache_file = cache_dir.join(format!("structure_{}.json", self.server_id));
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&cache_file, json)?;
        Ok(())
    }

    /// Loads the structure cache from disk.
    pub fn load(server_id: String, cache_dir: &Path) -> Result<Self> {
        let cache_file = cache_dir.join(format!("structure_{}.json", server_id));
        if !cache_file.exists() {
            return Ok(Self::new(server_id));
        }

        let json = std::fs::read_to_string(&cache_file)?;
        let mut cache: Self = serde_json::from_str(&json)?;

        // Ensure server_id matches (in case file was moved/renamed)
        cache.server_id = server_id;
        Ok(cache)
    }
}

/// Helper function to get current timestamp in seconds since Unix epoch.
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}

/// Helper function to format bytes in a human-readable way.
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_cached_jar_info_expiration() {
        let mod_info = ModInfo {
            id: "test".to_string(),
            name: "Test Mod".to_string(),
            version: Some("1.0.0".to_string()),
            file_path: PathBuf::from("test.jar"),
            enabled: true,
            side: crate::ModSide::Both,
            loader: crate::ModLoader::Unknown,
            raw_metadata: HashMap::new(),
        };

        let mut cached_info = CachedJarInfo::new(
            "hash123".to_string(),
            "test.jar".to_string(),
            1024,
            mod_info,
        );

        // Fresh entry should not be expired
        assert!(!cached_info.is_expired(24));

        // Simulate old cache entry
        cached_info.cached_at = current_timestamp() - 25 * 3600; // 25 hours ago
        assert!(cached_info.is_expired(24));
    }

    #[test]
    fn test_cache_stats() {
        let stats = CacheStats {
            entry_count: 10,
            total_size_bytes: 1024 * 1024,    // 1 MB
            max_size_bytes: 10 * 1024 * 1024, // 10 MB
        };

        assert_eq!(stats.usage_percentage(), 0.1);
        assert_eq!(stats.total_size_formatted(), "1.0 MB");
        assert_eq!(stats.max_size_formatted(), "10.0 MB");
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
    }
}
