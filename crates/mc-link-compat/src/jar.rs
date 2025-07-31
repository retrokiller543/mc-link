use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use serde::{Deserialize, Serialize};
use tracing::{debug, trace};
use zip::ZipArchive;
use mc_link_core::{ModInfo, ModSide, ModLoader};
use crate::{CompatError, Result};


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

/// Legacy Forge mcmod.info structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McModInfo {
    /// Mod ID
    #[serde(rename = "modid")]
    pub mod_id: String,
    /// Display name
    pub name: Option<String>,
    /// Version
    pub version: String,
    /// Description
    pub description: Option<String>,
    /// Authors
    pub authors: Option<Vec<String>>,
}

pub fn extract_jar_info<P: AsRef<Path>>(jar_path: P) -> Result<ModInfo> {
    use tracing::{info, warn};
    let jar_path = jar_path.as_ref();
    trace!(jar_path = %jar_path.display(), "Extracting jar file");

    let file = std::fs::File::open(&jar_path)?;
    let mut archive = ZipArchive::new(file)?;

    let mut metadata_files = Vec::new();
    for i in 0..archive.len() {
        if let Ok(file_in_jar) = archive.by_index(i) {
            let name = file_in_jar.name();
            if name.contains("META-INF") || name.ends_with(".toml") || name.ends_with(".json") || name.contains("mcmod") {
                trace!(file_name = name, "Found metadata file");
                metadata_files.push(name.to_string());
            }
        }
    }
    trace!(jar_path = %jar_path.display(), metadata_files = ?metadata_files, "JAR metadata scan complete");

    if let Ok(forge_info) = extract_mods_toml_info(&mut archive, jar_path) {
        debug!(jar_path = %jar_path.display(), mod_id = %forge_info.id, mod_name = %forge_info.name, version = ?forge_info.version, loader = ?forge_info.loader, "Extracted mod info from mods.toml");
        return Ok(forge_info);
    }

    if let Ok(fabric_info) = extract_fabric_info(&mut archive, jar_path) {
        info!(jar_path = %jar_path.display(), mod_id = %fabric_info.id, mod_name = %fabric_info.name, version = ?fabric_info.version, "Successfully extracted mod info from fabric.mod.json");
        return Ok(fabric_info);
    }

    if let Ok(mcmod_info) = extract_mcmod_info(&mut archive, jar_path) {
        info!(jar_path = %jar_path.display(), mod_id = %mcmod_info.id, mod_name = %mcmod_info.name, version = ?mcmod_info.version, "Successfully extracted mod info from mcmod.info");
        return Ok(mcmod_info);
    }

    if let Ok(manifest_info) = extract_manifest_info(&mut archive, jar_path) {
        info!(jar_path = %jar_path.display(), mod_id = %manifest_info.id, mod_name = %manifest_info.name, version = ?manifest_info.version, "Successfully extracted mod info from MANIFEST.MF");
        return Ok(manifest_info);
    }

    let filename = jar_path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown");
    warn!(jar_path = %jar_path.display(), filename = %filename, "Could not extract mod metadata, falling back to filename");

    Ok(ModInfo {
        id: filename.to_string(),
        name: filename.to_string(),
        version: Some("unknown".to_string()),
        file_path: jar_path.to_path_buf(),
        enabled: true,
        side: ModSide::Unknown,
        loader: ModLoader::Unknown,
        raw_metadata: HashMap::new(),
    })
}

fn extract_mods_toml_info(archive: &mut ZipArchive<std::fs::File>, jar_path: &Path) -> Result<ModInfo> {
    use tracing::{debug, warn};

    let contents = {
        let mut file = match archive.by_name("META-INF/mods.toml") {
            Ok(f) => f,
            Err(e) => {
                debug!("No META-INF/mods.toml found: {}", e);
                return Err(CompatError::MetadataError {
                    mod_name: "unknown".to_string(),
                    reason: format!("No mods.toml file: {}", e),
                });
            }
        };

        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        contents
    };

    debug!("Found mods.toml with {} bytes of content", contents.len());
    debug!("mods.toml content preview: {}", &contents[..contents.len().min(200)]);

    let manifest_version = read_manifest_version(archive);

    let forge_toml: ForgeModsToml = match toml::from_str(&contents) {
        Ok(toml) => toml,
        Err(e) => {
            warn!("Failed to parse mods.toml: {}", e);
            debug!("Full mods.toml content that failed to parse:\n{}", contents);
            return Err(CompatError::MetadataError {
                mod_name: "unknown".to_string(),
                reason: format!("TOML parse error: {}", e),
            });
        }
    };

    let forge_mod = forge_toml.mods.into_iter().next().ok_or_else(|| CompatError::MetadataError {
        mod_name: "unknown".to_string(),
        reason: "No mods found in mods.toml".to_string(),
    })?;

    let version = if forge_mod.version.trim() == "${file.jarVersion}" {
        manifest_version.unwrap_or_else(|| "unknown".to_string())
    } else {
        forge_mod.version
    };

    let side = parse_forge_side(&forge_mod.side);
    let loader = if contents.contains("javafml") || contents.contains("lowcodefml") {
        ModLoader::NeoForge
    } else if contents.contains("forge") || contents.contains("minecraftforge") {
        ModLoader::Forge
    } else {
        ModLoader::Unknown
    };

    Ok(ModInfo {
        id: forge_mod.mod_id.clone(),
        name: forge_mod.display_name.unwrap_or(forge_mod.mod_id.clone()),
        version: Some(version),
        file_path: jar_path.to_path_buf(),
        enabled: true,
        side,
        loader,
        raw_metadata: HashMap::new(),
    })
}

fn read_manifest_version(archive: &mut ZipArchive<std::fs::File>) -> Option<String> {
    if let Ok(mut file) = archive.by_name("META-INF/MANIFEST.MF") {
        let mut contents = String::new();
        if file.read_to_string(&mut contents).is_ok() {
            for line in contents.lines() {
                if let Some(value) = line.strip_prefix("Implementation-Version: ") {
                    return Some(value.trim().to_string());
                }
            }
        }
    }
    None
}

fn extract_fabric_info(archive: &mut ZipArchive<std::fs::File>, jar_path: &Path) -> Result<ModInfo> {
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
    
    Ok(ModInfo {
        id: fabric_info.id.clone(),
        name: fabric_info.name.unwrap_or(fabric_info.id),
        version: Some(fabric_info.version),
        file_path: jar_path.to_path_buf(),
        enabled: true,
        side,
        loader: ModLoader::Fabric,
        raw_metadata: if let serde_json::Value::Object(map) = raw {
            map.into_iter().collect()
        } else {
            HashMap::new()
        },
    })
}

fn extract_mcmod_info(archive: &mut ZipArchive<std::fs::File>, jar_path: &Path) -> Result<ModInfo> {
    use tracing::debug;
    
    // mcmod.info can be in root or META-INF
    let possible_paths = ["mcmod.info", "META-INF/mcmod.info"];
    
    let mut used_path = "";
    
    // Find which path exists
    for path in &possible_paths {
        if archive.by_name(path).is_ok() {
            used_path = path;
            break;
        }
    }
    
    if used_path.is_empty() {
        return Err(CompatError::MetadataError {
            mod_name: "unknown".to_string(),
            reason: "No mcmod.info file found".to_string(),
        });
    }
    
    // Now extract from the found path
    let mut file = archive.by_name(used_path)?;
    
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    
    debug!("Found mcmod.info at {} with {} bytes", used_path, contents.len());
    debug!("mcmod.info content preview: {}", &contents[..contents.len().min(200)]);
    
    // mcmod.info can be either a single object or an array
    let mcmod_info: McModInfo = if contents.trim_start().starts_with('[') {
        // Array format - take the first mod
        let mcmod_array: Vec<McModInfo> = serde_json::from_str(&contents)
            .map_err(|e| CompatError::MetadataError {
                mod_name: "unknown".to_string(),
                reason: format!("Failed to parse mcmod.info array: {}", e),
            })?;
        
        mcmod_array.into_iter().next()
            .ok_or_else(|| CompatError::MetadataError {
                mod_name: "unknown".to_string(),
                reason: "Empty mcmod.info array".to_string(),
            })?
    } else {
        // Single object format
        serde_json::from_str(&contents)
            .map_err(|e| CompatError::MetadataError {
                mod_name: "unknown".to_string(),
                reason: format!("Failed to parse mcmod.info object: {}", e),
            })?
    };
    
    Ok(ModInfo {
        id: mcmod_info.mod_id.clone(),
        name: mcmod_info.name.unwrap_or(mcmod_info.mod_id.clone()),
        version: Some(mcmod_info.version),
        file_path: jar_path.to_path_buf(),
        enabled: true,
        side: ModSide::Both, // mcmod.info doesn't typically specify side
        loader: ModLoader::Forge,
        raw_metadata: HashMap::new(),
    })
}

fn extract_manifest_info(archive: &mut ZipArchive<std::fs::File>, jar_path: &Path) -> Result<ModInfo> {
    use tracing::debug;
    
    let mut file = archive.by_name("META-INF/MANIFEST.MF")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    
    debug!("Found MANIFEST.MF with {} bytes", contents.len());
    debug!("MANIFEST.MF content preview: {}", &contents[..contents.len().min(300)]);
    
    // Parse manifest - simple key-value pairs separated by :
    let mut manifest_map = HashMap::new();
    for line in contents.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Some(colon_pos) = line.find(':') {
            let key = line[..colon_pos].trim();
            let value = line[colon_pos + 1..].trim();
            manifest_map.insert(key.to_lowercase(), value.to_string());
        }
    }
    
    // Look for common manifest attributes that might contain mod info
    let implementation_title = manifest_map.get("implementation-title");
    let implementation_version = manifest_map.get("implementation-version");
    let specification_title = manifest_map.get("specification-title");
    let specification_version = manifest_map.get("specification-version");
    let bundle_name = manifest_map.get("bundle-name");
    let bundle_version = manifest_map.get("bundle-version");
    
    // Try to derive mod ID and name from available info
    let mod_name = implementation_title
        .or(specification_title)
        .or(bundle_name)
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());
    
    let version = implementation_version
        .or(specification_version)
        .or(bundle_version)
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());
    
    // Convert name to a likely mod ID (lowercase, replace spaces with underscores)
    let mod_id = mod_name.to_lowercase()
        .replace(' ', "_")
        .replace('-', "_");
    
    // Only succeed if we found some meaningful info
    if mod_name == "unknown" && version == "unknown" {
        return Err(CompatError::MetadataError {
            mod_name: "unknown".to_string(),
            reason: "No useful mod metadata found in MANIFEST.MF".to_string(),
        });
    }
    
    Ok(ModInfo {
        id: mod_id,
        name: mod_name,
        version: Some(version),
        file_path: jar_path.to_path_buf(),
        enabled: true,
        side: ModSide::Both, // Can't determine from manifest
        loader: ModLoader::Unknown, // Can't determine from manifest
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