//! Compatibility profiles and resource management for Minecraft servers.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::error::{ConfigError, Result};
use crate::{config_struct, config_enum};

config_enum! {
    /// Type of compatibility profile.
    pub enum ProfileType {
        CompatibilityProfile,
        ModProfile,
        ServerTemplate,
    }
    default = CompatibilityProfile
}

config_enum! {
    /// Rule action for compatibility checking.
    pub enum RuleAction {
        Ignore,
        Require,
        ClientOnly,
        ServerOnly,
        Warn,
    }
    default = Ignore
}

config_struct! {
    /// Individual compatibility rule definition.
    pub struct CompatibilityRule {
        /// Mod ID or pattern this rule applies to
        pub mod_pattern: String = String::new(),
        /// Action to take for this mod
        pub action: RuleAction = RuleAction::default(),
        /// Reason for this rule (for documentation)
        pub reason: Option<String> = None,
        /// Whether this rule uses regex patterns
        pub is_regex: bool = false,
    }
}

config_struct! {
    /// Compatibility profile definition.
    pub struct CompatibilityProfile {
        /// Profile name/identifier
        pub name: String = String::new(),
        /// Profile description
        pub description: Option<String> = None,
        /// Profile version
        pub version: String = "1.0.0".to_string(),
        /// Author/creator
        pub author: Option<String> = None,
        /// Minecraft version this profile is for
        pub minecraft_version: Option<String> = None,
        /// Mod loader this profile is for
        pub mod_loader: Option<String> = None,
        /// Tags for categorization
        pub tags: Vec<String> = vec![],
        /// Compatibility rules
        pub rules: Vec<CompatibilityRule> = vec![],
        /// Whether this is a system profile (read-only)
        pub system: bool = false,
        /// Creation timestamp
        pub created_at: Option<String> = None,
        /// Last modified timestamp
        pub last_modified: Option<String> = None,
    }
}

config_struct! {
    /// Profile index - maps profile names to their metadata.
    pub struct ProfileIndex {
        /// Index version for compatibility
        pub version: String = "1.0.0".to_string(),
        /// Map of profile name to profile data
        pub profiles: HashMap<String, CompatibilityProfile> = HashMap::new(),
        /// Last index update timestamp
        pub last_updated: Option<String> = None,
    }
}

/// Profile manager handles loading, saving, and managing compatibility profiles.
#[derive(Debug, Clone)]
pub struct ProfileManager {
    profiles_dir: PathBuf,
    index: ProfileIndex,
}

impl ProfileManager {
    /// Creates a new profile manager.
    pub fn new(config_dir: &Path) -> Result<Self> {
        let profiles_dir = config_dir.join("profiles");
        
        // Create profiles directory if it doesn't exist
        std::fs::create_dir_all(&profiles_dir)
            .map_err(|e| ConfigError::io_error(
                "create profiles directory",
                format!("Failed to create profiles directory: {}", e),
                Some(e),
            ))?;
        
        // Load or create index
        let index = Self::load_index(&profiles_dir)?;
        
        let mut manager = Self {
            profiles_dir,
            index,
        };
        
        // Install default profiles if none exist
        manager.install_default_profiles()?;
        
        // Scan for profiles on initialization
        manager.scan_and_update()?;
        
        Ok(manager)
    }

    /// Loads profile index from disk.
    fn load_index(profiles_dir: &Path) -> Result<ProfileIndex> {
        let index_file = profiles_dir.join("index.json");
        
        if index_file.exists() {
            let content = std::fs::read_to_string(&index_file)
                .map_err(|e| ConfigError::io_error(
                    "read profile index",
                    format!("Failed to read profile index: {}", e),
                    Some(e),
                ))?;
            
            serde_json::from_str(&content)
                .map_err(|e| ConfigError::serialization_error(
                    "JSON",
                    format!("Failed to parse profile index: {}", e),
                    Some(Box::new(e)),
                ))
        } else {
            // Create default index
            let default_index = ProfileIndex::default();
            Self::save_index(&default_index, profiles_dir)?;
            Ok(default_index)
        }
    }

    /// Saves profile index to disk.
    fn save_index(index: &ProfileIndex, profiles_dir: &Path) -> Result<()> {
        let index_file = profiles_dir.join("index.json");
        
        let content = serde_json::to_string_pretty(index)
            .map_err(|e| ConfigError::serialization_error(
                "JSON",
                format!("Failed to serialize profile index: {}", e),
                Some(Box::new(e)),
            ))?;
        
        std::fs::write(&index_file, content)
            .map_err(|e| ConfigError::io_error(
                "write profile index",
                format!("Failed to write profile index: {}", e),
                Some(e),
            ))?;
        
        Ok(())
    }

    /// Gets a profile by name.
    pub fn get_profile(&self, name: &str) -> Option<&CompatibilityProfile> {
        self.index.profiles.get(name)
    }

    /// Lists all profile names.
    pub fn list_profiles(&self) -> Vec<String> {
        self.index.profiles.keys().cloned().collect()
    }

    /// Gets profiles by tag.
    pub fn get_profiles_by_tag(&self, tag: &str) -> Vec<&CompatibilityProfile> {
        self.index.profiles
            .values()
            .filter(|profile| profile.tags.contains(&tag.to_string()))
            .collect()
    }

    /// Adds or updates a profile.
    pub fn add_profile(&mut self, mut profile: CompatibilityProfile) -> Result<()> {
        // Update timestamps
        let now = chrono::Utc::now().to_rfc3339();
        if profile.created_at.is_none() {
            profile.created_at = Some(now.clone());
        }
        profile.last_modified = Some(now.clone());
        
        // Save profile to file
        let profile_file = self.profiles_dir.join(format!("{}.json", profile.name));
        let content = serde_json::to_string_pretty(&profile)
            .map_err(|e| ConfigError::serialization_error(
                "JSON",
                format!("Failed to serialize profile: {}", e),
                Some(Box::new(e)),
            ))?;
        
        std::fs::write(&profile_file, content)
            .map_err(|e| ConfigError::io_error(
                "write profile",
                format!("Failed to write profile file: {}", e),
                Some(e),
            ))?;
        
        // Update index
        self.index.profiles.insert(profile.name.clone(), profile);
        self.index.last_updated = Some(now);
        self.save()?;
        
        Ok(())
    }

    /// Removes a profile.
    pub fn remove_profile(&mut self, name: &str) -> Result<()> {
        let profile = self.get_profile(name)
            .ok_or_else(|| ConfigError::ProfileNotFound {
                profile_name: name.to_string(),
                cause: None,
            })?;
        
        // Don't allow removal of system profiles
        if profile.system {
            return Err(ConfigError::invalid_config(
                "profile.system",
                "Cannot remove system profile",
                None,
            ));
        }
        
        // Remove file
        let profile_file = self.profiles_dir.join(format!("{}.json", name));
        if profile_file.exists() {
            std::fs::remove_file(&profile_file)
                .map_err(|e| ConfigError::io_error(
                    "remove profile file",
                    format!("Failed to remove profile file: {}", e),
                    Some(e),
                ))?;
        }
        
        // Remove from index
        self.index.profiles.remove(name);
        self.index.last_updated = Some(chrono::Utc::now().to_rfc3339());
        self.save()?;
        
        Ok(())
    }

    /// Scans profiles directory and updates index.
    pub fn scan_and_update(&mut self) -> Result<()> {
        self.scan_profiles()?;
        self.save()?;
        Ok(())
    }

    /// Scans for profile files and updates index.
    fn scan_profiles(&mut self) -> Result<()> {
        for entry in std::fs::read_dir(&self.profiles_dir)
            .map_err(|e| ConfigError::io_error(
                "read profiles directory",
                format!("Failed to read profiles directory: {}", e),
                Some(e),
            ))?
        {
            let entry = entry
                .map_err(|e| ConfigError::io_error(
                    "read directory entry",
                    format!("Failed to read directory entry: {}", e),
                    Some(e),
                ))?;
            
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "json") {
                if path.file_name().map_or(true, |name| name != "index.json") {
                    self.process_profile_file(&path)?;
                }
            }
        }
        
        Ok(())
    }

    /// Processes a discovered profile file.
    fn process_profile_file(&mut self, profile_path: &Path) -> Result<()> {
        let content = std::fs::read_to_string(profile_path)
            .map_err(|e| ConfigError::io_error(
                "read profile file",
                format!("Failed to read profile file: {}", e),
                Some(e),
            ))?;

        let profile: CompatibilityProfile = serde_json::from_str(&content)
            .map_err(|e| ConfigError::serialization_error(
                "JSON",
                format!("Failed to parse profile file: {}", e),
                Some(Box::new(e)),
            ))?;

        // Update index if not already present or if file is newer
        self.index.profiles.insert(profile.name.clone(), profile);
        
        Ok(())
    }

    /// Saves index to disk.
    pub fn save(&self) -> Result<()> {
        Self::save_index(&self.index, &self.profiles_dir)
    }

    /// Reloads index from disk.
    pub fn reload(&mut self) -> Result<()> {
        self.index = Self::load_index(&self.profiles_dir)?;
        Ok(())
    }

    /// Installs default builtin profiles if none exist.
    fn install_default_profiles(&mut self) -> Result<()> {
        // Only install if no profiles exist
        if !self.index.profiles.is_empty() {
            return Ok(());
        }

        // Create a basic default profile
        let default_profile = CompatibilityProfile {
            name: "default".to_string(),
            description: Some("Default compatibility profile with common rules".to_string()),
            version: "1.0.0".to_string(),
            author: Some("mc-link".to_string()),
            minecraft_version: None,
            mod_loader: None,
            tags: vec!["default".to_string(), "builtin".to_string()],
            rules: vec![
                CompatibilityRule {
                    mod_pattern: "optifine".to_string(),
                    action: RuleAction::ClientOnly,
                    reason: Some("OptiFine is client-only".to_string()),
                    is_regex: false,
                },
                CompatibilityRule {
                    mod_pattern: ".*-client$".to_string(),
                    action: RuleAction::ClientOnly,
                    reason: Some("Mods ending with -client are typically client-only".to_string()),
                    is_regex: true,
                },
            ],
            system: true,
            created_at: Some(chrono::Utc::now().to_rfc3339()),
            last_modified: Some(chrono::Utc::now().to_rfc3339()),
        };

        self.add_profile(default_profile)?;
        
        Ok(())
    }

    /// Gets the profiles directory path.
    pub fn profiles_dir(&self) -> &Path {
        &self.profiles_dir
    }
}