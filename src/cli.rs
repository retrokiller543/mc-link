use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// MC-Link: Minecraft server mod synchronization tool
#[derive(Parser)]
#[command(name = "mc-link")]
#[command(about = "A tool for managing and synchronizing Minecraft server mods")]
#[command(version)]
pub struct Cli {
    /// Enable verbose logging
    #[arg(short, long, global = true)]
    pub verbose: bool,
    
    /// Custom configuration directory
    #[arg(short, long, global = true)]
    pub config_dir: Option<PathBuf>,
    
    #[command(subcommand)]
    pub command: Commands,
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
    Add {
        /// Unique identifier for the server/client
        id: String,
        
        /// Human-readable name
        name: String,
        
        /// Connection type (local, ftp)
        #[arg(short, long)]
        connection_type: String,
        
        /// Connection details (path for local, host:port for ftp)
        #[arg(short, long)]
        target: String,
        
        /// FTP username (for FTP connections only)
        #[arg(short, long)]
        username: Option<String>,
        
        /// FTP password (for FTP connections only)  
        #[arg(short, long)]
        password: Option<String>,
        
        /// Minecraft version
        #[arg(long, default_value = "1.21.1")]
        minecraft_version: String,
        
        /// Mod loader (NeoForge, Forge, Fabric)
        #[arg(long, default_value = "NeoForge")]
        mod_loader: String,
        
        /// Disable the server/client by default
        #[arg(long)]
        disabled: bool,
    },
    
    /// Remove a server or client configuration
    Remove {
        /// Server/client ID to remove
        id: String,
        
        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,
    },
    
    /// Scan a server or client for mods
    Scan {
        /// Server/client ID to scan
        id: String,
        
        /// Show detailed mod information
        #[arg(short, long)]
        detailed: bool,
    },
    
    /// Compare two instances and show sync plan
    Compare {
        /// Source (client) ID
        source: String,
        
        /// Target (server) ID
        target: String,
        
        /// Show detailed comparison results
        #[arg(short, long)]
        detailed: bool,
    },
    
    /// Synchronize mods from source to target
    Sync {
        /// Source (client) ID
        source: String,
        
        /// Target (server) ID
        target: String,
        
        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,
        
        /// Dry run - show what would be done without executing
        #[arg(long)]
        dry_run: bool,
    },
    
    /// Enable or disable a server/client
    Toggle {
        /// Server/client ID to toggle
        id: String,
        
        /// Set enabled state explicitly
        #[arg(short, long)]
        enabled: Option<bool>,
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