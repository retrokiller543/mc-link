use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]

pub enum ConnectionType {
    Local,
    Ftp,
}

/// MC-Link: Minecraft server mod synchronization tool
#[derive(Parser)]
#[command(
    version,
    name = "mc-link",
    about = "A tool for managing and synchronizing Minecraft server mods",
    long_about = "MC-Link helps you manage and synchronize Minecraft server mods between different instances.\n\nTIP: Most commands support interactive TUI mode when run without required arguments for a better user experience."
)]
pub struct Cli {
    /// Enable verbose logging
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Custom configuration directory
    #[arg(short, long, global = true)]
    pub config_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// List all configured servers and clients
    List {
        /// Show only enabled servers
        #[arg(short, long)]
        enabled_only: bool,
    },

    /// Add a new server or client configuration
    /// Uses interactive TUI mode when required arguments are missing
    Add {
        #[command(flatten)]
        config: AddServerConfig,
    },

    /// Remove a server or client configuration
    /// Uses interactive selection when ID is not provided
    Remove {
        #[command(flatten)]
        target: RemoveTarget,
    },

    /// Scan a server or client for mods
    /// Uses interactive selection when ID is not provided
    Scan {
        #[command(flatten)]
        target: ScanTarget,
    },

    /// Compare two instances and show sync plan
    /// Uses interactive selection when source/target not provided
    Compare {
        #[command(flatten)]
        targets: CompareTargets,
    },

    /// Synchronize mods from source to target
    /// Uses interactive selection when source/target not provided
    Sync {
        #[command(flatten)]
        targets: SyncTargets,
    },

    /// Enable or disable a server/client
    /// Uses interactive selection when ID is not provided
    Toggle {
        #[command(flatten)]
        target: ToggleTarget,
    },

    /// Show configuration details
    Config {
        /// Server/client ID to show (shows all if not specified)
        id: Option<String>,

        /// Show in JSON format
        #[arg(short, long)]
        json: bool,
    },
}

/// Server configuration for adding new servers
#[derive(Args)]
#[group(required = false, multiple = true)]
pub struct AddServerConfig {
    /// Unique identifier for the server/client
    #[arg(group = "required_config")]
    pub id: Option<String>,

    /// Human-readable name
    #[arg(group = "required_config")]
    pub name: Option<String>,

    /// Connection type (local, ftp)
    #[arg(short = 't', long, group = "required_config", value_enum)]
    pub connection_type: Option<ConnectionType>,

    /// Connection details (path for local, host:port for ftp)
    #[arg(short, long, group = "required_config")]
    pub target: Option<String>,

    /// FTP username (for FTP connections only)
    #[arg(short, long)]
    pub username: Option<String>,

    /// FTP password (for FTP connections only)  
    #[arg(short, long)]
    pub password: Option<String>,

    /// Minecraft version
    #[arg(long, default_value = "1.21.1")]
    pub minecraft_version: String,

    /// Mod loader (NeoForge, Forge, Fabric)
    #[arg(long, default_value = "NeoForge")]
    pub mod_loader: String,

    /// Disable the server/client by default
    #[arg(long)]
    pub disabled: bool,
}

impl AddServerConfig {
    /// Returns true if interactive mode should be used (missing required args)
    pub fn is_interactive(&self) -> bool {
        self.id.is_none()
            || self.name.is_none()
            || self.connection_type.is_none()
            || self.target.is_none()
    }
}

/// Target for remove operations
#[derive(Args)]
pub struct RemoveTarget {
    /// Server/client ID to remove
    pub id: Option<String>,

    /// Skip confirmation prompt
    #[arg(short, long)]
    pub force: bool,
}

impl RemoveTarget {
    pub fn is_interactive(&self) -> bool {
        self.id.is_none()
    }
}

/// Target for scan operations
#[derive(Args)]
pub struct ScanTarget {
    /// Server/client ID to scan
    pub id: Option<String>,

    /// Show detailed mod information
    #[arg(short, long)]
    pub detailed: bool,
}

impl ScanTarget {
    pub fn is_interactive(&self) -> bool {
        self.id.is_none()
    }
}

/// Source and target for compare operations
#[derive(Args)]
pub struct CompareTargets {
    /// Source (client) ID
    pub source: Option<String>,

    /// Target (server) ID
    pub target: Option<String>,

    /// Show detailed comparison results
    #[arg(short, long)]
    pub detailed: bool,
}

impl CompareTargets {
    pub fn is_interactive(&self) -> bool {
        self.source.is_none() || self.target.is_none()
    }
}

/// Source and target for sync operations
#[derive(Args)]
pub struct SyncTargets {
    /// Source (client) ID
    pub source: Option<String>,

    /// Target (server) ID  
    pub target: Option<String>,

    /// Skip confirmation prompt
    #[arg(short, long)]
    pub force: bool,

    /// Dry run - show what would be done without executing
    #[arg(long)]
    pub dry_run: bool,
}

impl SyncTargets {
    pub fn is_interactive(&self) -> bool {
        self.source.is_none() || self.target.is_none()
    }
}

/// Target for toggle operations
#[derive(Args)]
pub struct ToggleTarget {
    /// Server/client ID to toggle
    pub id: Option<String>,

    /// Set enabled state explicitly
    #[arg(short, long)]
    pub enabled: Option<bool>,
}

impl ToggleTarget {
    pub fn is_interactive(&self) -> bool {
        self.id.is_none()
    }
}
