use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use serde::{Deserialize, Serialize};
use zip::ZipArchive;
use crate::{CompatError, Result};

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

/// Fabric mod metadata from fabric.mod.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FabricModInfo {
    /// Mod ID
    pub id: String,
    /// Display name
    pub name: Option<String>,
    /// Version
    pub version: String,
    /// Environment the mod runs in
    pub environment: Option<String>,
}

/// NeoForge/Forge mods.toml structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeModsToml {
    /// Array of mods defined in this file
    #[serde(rename = "mods")]
    pub mods: Vec<ForgeModInfo>,
}

/// Individual NeoForge/Forge mod metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeModInfo {
    /// Mod ID
    #[serde(rename = "modId")]
    pub mod_id: String,
    /// Display name
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    /// Version
    pub version: String,
    /// Side specification
    pub side: Option<String>,
}

/// Extracted metadata from a mod JAR file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JarModInfo {
    /// Mod ID
    pub id: String,
    /// Display name
    pub name: String,
    /// Version string
    pub version: String,
    /// Which side(s) the mod runs on
    pub side: ModSide,
    /// Mod loader type
    pub loader: ModLoader,
    /// Raw metadata for advanced processing
    pub raw_metadata: HashMap<String, serde_json::Value>,
}

/// Supported mod loaders in priority order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModLoader {
    /// NeoForge mod loader (primary focus)
    NeoForge,
    /// Fabric mod loader (secondary)
    Fabric,
    /// Legacy Forge mod loader (fallback)
    Forge,
    /// Unknown or unsupported mod loader
    Unknown,
}

/// Extracts mod information from a JAR file.
///
/// Priority order: NeoForge -> Fabric -> Forge -> filename fallback
pub fn extract_jar_info<P: AsRef<Path>>(jar_path: P) -> Result<JarModInfo> {
    let file = std::fs::File::open(&jar_path)?;
    let mut archive = ZipArchive::new(file)?;
    
    // Try NeoForge first (primary focus)
    if let Ok(neoforge_info) = extract_neoforge_info(&mut archive) {
        return Ok(neoforge_info);
    }
    
    // Try Fabric second
    if let Ok(fabric_info) = extract_fabric_info(&mut archive) {
        return Ok(fabric_info);
    }
    
    // Try legacy Forge third
    if let Ok(forge_info) = extract_forge_info(&mut archive) {
        return Ok(forge_info);
    }
    
    // Fallback to filename-based detection
    let path = jar_path.as_ref();
    let filename = path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");
    
    Ok(JarModInfo {
        id: filename.to_string(),
        name: filename.to_string(),
        version: "unknown".to_string(),
        side: ModSide::Unknown,
        loader: ModLoader::Unknown,
        raw_metadata: HashMap::new(),
    })
}

fn extract_neoforge_info(archive: &mut ZipArchive<std::fs::File>) -> Result<JarModInfo> {
    // NeoForge uses the same mods.toml format as Forge but may have different markers
    let mut file = archive.by_name("META-INF/mods.toml")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    
    // Check if this is actually a NeoForge mod by looking for NeoForge-specific markers
    let is_neoforge = contents.contains("neoforge") || 
                     contents.contains("NeoForge") ||
                     contents.contains("net.neoforged");
    
    if !is_neoforge {
        return Err(CompatError::MetadataError {
            mod_name: "unknown".to_string(),
            reason: "Not a NeoForge mod".to_string(),
        });
    }
    
    let forge_toml: ForgeModsToml = toml::from_str(&contents)?;
    
    let forge_mod = forge_toml.mods.into_iter().next()
        .ok_or_else(|| CompatError::MetadataError {
            mod_name: "unknown".to_string(),
            reason: "No mods found in mods.toml".to_string(),
        })?;
    
    let side = parse_forge_side(&forge_mod.side);
    
    Ok(JarModInfo {
        id: forge_mod.mod_id.clone(),
        name: forge_mod.display_name.unwrap_or(forge_mod.mod_id),
        version: forge_mod.version,
        side,
        loader: ModLoader::NeoForge,
        raw_metadata: HashMap::new(),
    })
}

fn extract_fabric_info(archive: &mut ZipArchive<std::fs::File>) -> Result<JarModInfo> {
    let mut file = archive.by_name("fabric.mod.json")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    
    let fabric_info: FabricModInfo = serde_json::from_str(&contents)?;
    let raw: serde_json::Value = serde_json::from_str(&contents)?;
    
    let side = match fabric_info.environment.as_deref() {
        Some("client") => ModSide::Client,
        Some("server") => ModSide::Server,
        Some("*") | None => ModSide::Both,
        _ => ModSide::Unknown,
    };
    
    Ok(JarModInfo {
        id: fabric_info.id.clone(),
        name: fabric_info.name.unwrap_or(fabric_info.id),
        version: fabric_info.version,
        side,
        loader: ModLoader::Fabric,
        raw_metadata: if let serde_json::Value::Object(map) = raw {
            map.into_iter().collect()
        } else {
            HashMap::new()
        },
    })
}

fn extract_forge_info(archive: &mut ZipArchive<std::fs::File>) -> Result<JarModInfo> {
    let mut file = archive.by_name("META-INF/mods.toml")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    
    let forge_toml: ForgeModsToml = toml::from_str(&contents)?;
    
    let forge_mod = forge_toml.mods.into_iter().next()
        .ok_or_else(|| CompatError::MetadataError {
            mod_name: "unknown".to_string(),
            reason: "No mods found in mods.toml".to_string(),
        })?;
    
    let side = parse_forge_side(&forge_mod.side);
    
    Ok(JarModInfo {
        id: forge_mod.mod_id.clone(),
        name: forge_mod.display_name.unwrap_or(forge_mod.mod_id),
        version: forge_mod.version,
        side,
        loader: ModLoader::Forge,
        raw_metadata: HashMap::new(),
    })
}

fn parse_forge_side(side: &Option<String>) -> ModSide {
    match side.as_deref() {
        Some("CLIENT") => ModSide::Client,
        Some("SERVER") => ModSide::Server,
        Some("BOTH") | None => ModSide::Both,
        _ => ModSide::Unknown,
    }
}