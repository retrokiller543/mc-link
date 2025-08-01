use crate::Result;
use mc_link_core::{ModInfo, ModSide};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Configuration for mod compatibility checking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatConfig {
    /// Mod IDs to ignore during compatibility checking
    pub ignore_list: HashSet<String>,
    /// Custom compatibility rules
    pub custom_rules: Vec<CompatRule>,
    /// Whether to automatically ignore client-only mods on server
    pub auto_ignore_client_only: bool,
    /// Whether to automatically ignore server-only mods on client
    pub auto_ignore_server_only: bool,
}

impl Default for CompatConfig {
    fn default() -> Self {
        Self {
            ignore_list: HashSet::new(),
            custom_rules: Vec::new(),
            auto_ignore_client_only: true,
            auto_ignore_server_only: true,
        }
    }
}

/// Custom compatibility rule for specific mod combinations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatRule {
    /// Mod ID this rule applies to
    pub mod_id: String,
    /// Rule type
    pub rule_type: RuleType,
    /// Human-readable reason for the rule
    pub reason: String,
}

/// Types of compatibility rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuleType {
    /// Always ignore this mod
    AlwaysIgnore,
    /// Require this mod to be present on both sides
    RequireBoth,
    /// Only allow on client
    ClientOnly,
    /// Only allow on server
    ServerOnly,
}

/// Result of a compatibility check between client and server mods.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatResult {
    /// Mods that are missing on the server but present on client
    pub missing_on_server: Vec<ModInfo>,
    /// Mods that are missing on the client but present on server
    pub missing_on_client: Vec<ModInfo>,
    /// Mods with version mismatches
    pub version_mismatches: Vec<VersionMismatch>,
    /// Mods that were ignored during the check
    pub ignored_mods: Vec<String>,
    /// Overall compatibility status
    pub is_compatible: bool,
}

/// Information about a version mismatch between client and server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionMismatch {
    /// Mod ID
    pub mod_id: String,
    /// Mod name
    pub mod_name: String,
    /// Version on client
    pub client_version: String,
    /// Version on server
    pub server_version: String,
}

/// Checks compatibility between client and server mod lists.
pub fn check_compatibility(
    client_mods: &[ModInfo],
    server_mods: &[ModInfo],
    config: &CompatConfig,
) -> Result<CompatResult> {
    use tracing::{debug, info};

    info!(
        client_mod_count = client_mods.len(),
        server_mod_count = server_mods.len(),
        "Starting a detailed compatibility check"
    );

    let mut result = CompatResult {
        missing_on_server: Vec::new(),
        missing_on_client: Vec::new(),
        version_mismatches: Vec::new(),
        ignored_mods: Vec::new(),
        is_compatible: true,
    };

    // Create lookup maps for efficient comparison using the proper mod IDs
    let client_map: HashMap<String, &ModInfo> = client_mods
        .iter()
        .map(|m| {
            debug!(
                mod_id = %m.id,
                mod_name = %m.name,
                file_path = %m.file_path.display(),
                "Client mod mapping"
            );
            (m.id.clone(), m)
        })
        .collect();

    let server_map: HashMap<String, &ModInfo> = server_mods
        .iter()
        .map(|m| {
            debug!(
                mod_id = %m.id,
                mod_name = %m.name,
                file_path = %m.file_path.display(),
                "Server mod mapping"
            );
            (m.id.clone(), m)
        })
        .collect();

    info!(
        unique_client_ids = client_map.len(),
        unique_server_ids = server_map.len(),
        "Created mod ID mappings"
    );

    // Check each client mod
    for client_mod in client_mods {
        let mod_id = &client_mod.id;

        // Skip if in ignore list
        if config.ignore_list.contains(mod_id) {
            result.ignored_mods.push(mod_id.clone());
            continue;
        }

        // Apply custom rules
        if let Some(rule) = config.custom_rules.iter().find(|r| r.mod_id == *mod_id) {
            match rule.rule_type {
                RuleType::AlwaysIgnore => {
                    result.ignored_mods.push(mod_id.clone());
                    continue;
                }
                RuleType::ClientOnly => {
                    result.ignored_mods.push(mod_id.clone());
                    continue;
                }
                RuleType::ServerOnly => {
                    result.missing_on_server.push(client_mod.clone());
                    result.is_compatible = false;
                    continue;
                }
                RuleType::RequireBoth => {
                    // Continue with normal checking
                }
            }
        }

        // Auto-ignore based on mod side information
        match client_mod.side {
            ModSide::Client if config.auto_ignore_client_only => {
                result.ignored_mods.push(mod_id.clone());
                continue;
            }
            ModSide::Server => {
                result.missing_on_server.push(client_mod.clone());
                result.is_compatible = false;
                continue;
            }
            _ => {}
        }

        // Check if mod exists on server
        if let Some(server_mod) = server_map.get(mod_id) {
            // Check version compatibility
            if client_mod.version != server_mod.version {
                if let (Some(client_ver), Some(server_ver)) =
                    (&client_mod.version, &server_mod.version)
                {
                    result.version_mismatches.push(VersionMismatch {
                        mod_id: mod_id.clone(),
                        mod_name: client_mod.name.clone(),
                        client_version: client_ver.clone(),
                        server_version: server_ver.clone(),
                    });
                    result.is_compatible = false;
                }
            }
        } else {
            result.missing_on_server.push(client_mod.clone());
            result.is_compatible = false;
        }
    }

    // Check each server mod for client-missing mods
    for server_mod in server_mods {
        let mod_id = &server_mod.id;

        // Skip if in ignore list or already processed
        if config.ignore_list.contains(mod_id) || client_map.contains_key(mod_id) {
            continue;
        }

        // Apply custom rules
        if let Some(rule) = config.custom_rules.iter().find(|r| r.mod_id == *mod_id) {
            match rule.rule_type {
                RuleType::AlwaysIgnore | RuleType::ServerOnly => continue,
                RuleType::ClientOnly => {
                    result.missing_on_client.push(server_mod.clone());
                    result.is_compatible = false;
                    continue;
                }
                RuleType::RequireBoth => {
                    // Continue with normal checking
                }
            }
        }

        // Auto-ignore based on mod side information
        match server_mod.side {
            ModSide::Server if config.auto_ignore_server_only => continue,
            ModSide::Client => {
                result.missing_on_client.push(server_mod.clone());
                result.is_compatible = false;
                continue;
            }
            _ => {}
        }

        result.missing_on_client.push(server_mod.clone());
        result.is_compatible = false;
    }

    Ok(result)
}
