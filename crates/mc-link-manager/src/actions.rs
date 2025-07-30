use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use mc_link_core::ModInfo;

/// Represents an action needed to synchronize two Minecraft instances.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncAction {
    /// Update an existing mod to a different version
    UpdateMod {
        /// Mod identifier
        mod_id: String,
        /// Current version
        from_version: String,
        /// Target version
        to_version: String,
        /// Path to the current mod file
        current_path: PathBuf,
        /// Path to the new mod file
        new_path: PathBuf,
    },
    
    /// Add a new mod that doesn't exist on the target
    AddMod {
        /// Mod information to add
        mod_info: ModInfo,
        /// Target location (Client or Server)
        target: SyncTarget,
    },
    
    /// Remove a mod that shouldn't exist on the target
    RemoveMod {
        /// Mod identifier
        mod_id: String,
        /// Mod information
        mod_info: ModInfo,
        /// Target location (Client or Server)
        target: SyncTarget,
    },
    
    /// Keep mod as-is (no action needed)
    KeepAsIs {
        /// Mod identifier
        mod_id: String,
        /// Reason for keeping as-is
        reason: String,
    },
}

/// Target for sync operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncTarget {
    /// Action should be performed on the client
    Client,
    /// Action should be performed on the server
    Server,
}

/// Result of comparing two Minecraft instances.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncPlan {
    /// List of actions needed to synchronize
    pub actions: Vec<SyncAction>,
    /// Summary of what will be changed
    pub summary: SyncSummary,
    /// Whether the sync would result in compatibility
    pub will_be_compatible: bool,
}

/// Summary of planned sync operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSummary {
    /// Number of mods to be updated
    pub mods_to_update: usize,
    /// Number of mods to be added
    pub mods_to_add: usize,
    /// Number of mods to be removed
    pub mods_to_remove: usize,
    /// Number of mods to keep as-is
    pub mods_to_keep: usize,
    /// Total number of mods processed
    pub total_mods: usize,
}

impl SyncPlan {
    /// Creates a new empty sync plan.
    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
            summary: SyncSummary {
                mods_to_update: 0,
                mods_to_add: 0,
                mods_to_remove: 0,
                mods_to_keep: 0,
                total_mods: 0,
            },
            will_be_compatible: true,
        }
    }
    
    /// Adds an action to the sync plan and updates the summary.
    pub fn add_action(&mut self, action: SyncAction) {
        match &action {
            SyncAction::UpdateMod { .. } => self.summary.mods_to_update += 1,
            SyncAction::AddMod { .. } => self.summary.mods_to_add += 1,
            SyncAction::RemoveMod { .. } => self.summary.mods_to_remove += 1,
            SyncAction::KeepAsIs { .. } => self.summary.mods_to_keep += 1,
        }
        self.summary.total_mods += 1;
        self.actions.push(action);
    }
    
    /// Returns true if the sync plan has any actions to perform.
    pub fn has_changes(&self) -> bool {
        self.summary.mods_to_update > 0 
            || self.summary.mods_to_add > 0 
            || self.summary.mods_to_remove > 0
    }
}