use mc_link_core::ModInfo;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Represents the standard Minecraft server/client directory structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftStructure {
    /// Root path of the Minecraft instance
    pub root_path: PathBuf,
    /// Mods directory and its contents
    pub mods: ModsStructure,
    /// Config directory (planned for future)
    pub config: ConfigStructure,
    /// Resource packs directory (planned for future)
    pub resourcepacks: ResourcePackStructure,
    /// Shader packs directory (planned for future)
    pub shaderpacks: ShaderPackStructure,
    /// Server properties and other root files
    pub root_files: HashMap<String, PathBuf>,
}

/// Structure of the mods directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModsStructure {
    /// Path to the mods directory
    pub path: PathBuf,
    /// List of installed mods with their metadata
    pub mods: Vec<ModInfo>,
    /// Whether the mods directory exists
    pub exists: bool,
}

/// Structure of the config directory (placeholder for future implementation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigStructure {
    /// Path to the config directory
    pub path: PathBuf,
    /// Whether the config directory exists
    pub exists: bool,
    /// Config files (to be implemented)
    pub files: Vec<PathBuf>,
}

/// Structure of the resourcepacks directory (placeholder for future implementation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcePackStructure {
    /// Path to the resourcepacks directory
    pub path: PathBuf,
    /// Whether the resourcepacks directory exists
    pub exists: bool,
    /// Resource pack files (to be implemented)
    pub packs: Vec<PathBuf>,
}

/// Structure of the shaderpacks directory (placeholder for future implementation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderPackStructure {
    /// Path to the shaderpacks directory
    pub path: PathBuf,
    /// Whether the shaderpacks directory exists
    pub exists: bool,
    /// Shader pack files (to be implemented)
    pub packs: Vec<PathBuf>,
}

impl MinecraftStructure {
    /// Creates a new Minecraft structure for the given root path.
    pub fn new(root_path: PathBuf) -> Self {
        Self {
            mods: ModsStructure {
                path: PathBuf::from("mods"),
                mods: Vec::new(),
                exists: false,
            },
            config: ConfigStructure {
                path: root_path.join("config"),
                exists: false,
                files: Vec::new(),
            },
            resourcepacks: ResourcePackStructure {
                path: root_path.join("resourcepacks"),
                exists: false,
                packs: Vec::new(),
            },
            shaderpacks: ShaderPackStructure {
                path: root_path.join("shaderpacks"),
                exists: false,
                packs: Vec::new(),
            },
            root_files: HashMap::new(),
            root_path,
        }
    }

    /// Returns the total number of mods installed.
    pub fn mod_count(&self) -> usize {
        self.mods.mods.len()
    }

    /// Checks if the mods directory exists and has mods.
    pub fn has_mods(&self) -> bool {
        self.mods.exists && !self.mods.mods.is_empty()
    }
}
