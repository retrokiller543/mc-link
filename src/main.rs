mod cli;

use mc_link_config::CONFIG_MANAGER;
use mc_link_manager::MinecraftManager;
use tracing::{info, error};
use mc_link_manager::prelude::CompatConfig;

const SERVER_ID: &str = "Cogwork";
const CLIENT_ID: &str = "TestClient";

#[tokio::main]
async fn main() {
    // Initialize the configuration manager
    let config_manager = &CONFIG_MANAGER;
    config_manager.save().unwrap();

    let log_dir = config_manager.config_dir().join("logs");
    if !log_dir.exists() {
        std::fs::create_dir_all(&log_dir).unwrap();
    }
    
    // Initialize logging
    let _guards = mc_link_core::logging::tracing(&log_dir, &config_manager.manager)
        .expect("Failed to initialize logging");

    info!("Using configuration directory: {}", config_manager.config_dir().display());

    // Get the default server configuration
    let server_config = config_manager.get_server(SERVER_ID).expect("Default server not found");
    let client_config = config_manager.get_server(CLIENT_ID).expect("Client not found");

    // Create a Minecraft manager from the server configuration
    let mut client_manager = MinecraftManager::from_config(client_config);
    let mut server_manager = MinecraftManager::from_config(server_config);

    // Scan the client structure
    info!("Scanning client: {}...", client_config.name);
    match client_manager.scan().await {
        Ok(structure) => {
            info!("Client scan complete!");
            info!("Found {} mods on client.", structure.mods.mods.len());
        }
        Err(e) => {
            error!("Failed to scan client: {}", e);
            return;
        }
    }
    
    // Scan the server structure
    info!("Scanning server: {}...", server_config.name);
    match server_manager.scan().await {
        Ok(structure) => {
            info!("Server scan complete!");
            info!("Found {} mods on server.", structure.mods.mods.len());
        }
        Err(e) => {
            error!("Failed to scan server: {}", e);
            return;
        }
    }
    
    let compat_config = CompatConfig::default();
    
    let plan = client_manager.compare_with(&mut server_manager, &compat_config).await.unwrap();
    
    info!("Comparison plan: {:#?}", plan);
}
