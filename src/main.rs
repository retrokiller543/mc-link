use mc_link_config::CONFIG_MANAGER;
use mc_link_manager::MinecraftManager;
use tracing::{info, error};
use mc_link_manager::prelude::CompatConfig;

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
    let default_server_id = config_manager.servers().default_server.as_deref().unwrap_or("Cogwork");
    let server_config = config_manager.get_server(default_server_id).expect("Default server not found");

    // Create a Minecraft manager from the server configuration
    let mut manager = MinecraftManager::from_config(server_config);
    let mut manager2 = MinecraftManager::from_config(server_config);

    // Scan the server structure
    info!("Scanning server: {}...", server_config.name);
    match manager.scan().await {
        Ok(structure) => {
            info!("Scan complete!");
            info!("Found {} mods.", structure.mods.mods.len());
        }
        Err(e) => {
            error!("Failed to scan server: {}", e);
        }
    }
    
    let compat_config = CompatConfig::default();
    
    let plan = manager.compare_with(&mut manager2, &compat_config).await.unwrap();
    
    info!("Comparison plan: {:#?}", plan);
}
