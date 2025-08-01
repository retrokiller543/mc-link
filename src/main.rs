mod cli;
mod tui;

use clap::Parser;
use cli::{Cli, Commands};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use mc_link_config::{CONFIG_MANAGER, ConfigManager};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;
use tracing::info;
use tui::App;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse CLI arguments
    let cli = Cli::parse();

    // Initialize the configuration manager
    let config_manager = &CONFIG_MANAGER;

    let log_dir = config_manager.config_dir().join("logs");
    if !log_dir.exists() {
        std::fs::create_dir_all(&log_dir).unwrap();
    }

    // Initialize logging
    let _guards = mc_link_core::logging::tracing(&log_dir, &config_manager.manager)
        .expect("Failed to initialize logging");

    info!(
        "Using configuration directory: {}",
        config_manager.config_dir().display()
    );

    // Handle CLI commands or start TUI
    match cli.command {
        Some(command) => handle_cli_command(command, config_manager).await?,
        None => run_tui().await?,
    }

    Ok(())
}

async fn handle_cli_command(
    command: Commands,
    config: &ConfigManager,
) -> Result<(), Box<dyn std::error::Error>> {
    use mc_link_config::{ConnectionType, FtpConnection, LocalConnection, ServerConfig};
    use mc_link_manager::MinecraftManager;

    match command {
        Commands::List { enabled_only } => {
            let servers = config.list_servers();
            if servers.is_empty() {
                println!("No servers configured.");
                return Ok(());
            }

            println!("Configured servers:");
            for server_id in servers {
                if let Some(server) = config.get_server(server_id) {
                    if enabled_only && !server.enabled {
                        continue;
                    }
                    let status = if server.enabled { "✓" } else { "✗" };
                    let conn_type = server.connection.type_name();
                    println!(
                        "  {} {} [{}] - {}",
                        status, server_id, conn_type, server.name
                    );
                }
            }
        }
        Commands::Add { config: add_config } => {
            if add_config.is_interactive() {
                println!("Starting interactive mode for adding server...");
                run_add_server_tui().await?;
            } else {
                // Non-interactive server addition
                let mut config_manager = mc_link_config::ConfigManager::new()?;

                let connection = match add_config.connection_type.as_ref().unwrap() {
                    crate::cli::ConnectionType::Local => ConnectionType::Local(LocalConnection {
                        path: add_config.target.as_ref().unwrap().clone(),
                    }),
                    crate::cli::ConnectionType::Ftp => {
                        let target = add_config.target.as_ref().unwrap();
                        let (host, port) = if target.contains(':') {
                            let parts: Vec<&str> = target.split(':').collect();
                            (
                                parts[0].to_string(),
                                parts.get(1).unwrap_or(&"21").parse().unwrap_or(21),
                            )
                        } else {
                            (target.clone(), 21)
                        };

                        ConnectionType::Ftp(FtpConnection {
                            host,
                            port,
                            username: add_config.username.clone().unwrap_or_default(),
                            password: add_config.password.clone(),
                            ..Default::default()
                        })
                    }
                };

                let mut server_config = ServerConfig::new(
                    add_config.id.as_ref().unwrap().clone(),
                    add_config.name.as_ref().unwrap().clone(),
                );

                server_config.connection = connection;
                server_config.enabled = !add_config.disabled;
                server_config.settings.minecraft_version =
                    Some(add_config.minecraft_version.clone());

                // Parse mod loader
                use mc_link_config::ModLoader;
                server_config.settings.mod_loader = match add_config.mod_loader.as_str() {
                    "NeoForge" => ModLoader::NeoForge,
                    "Forge" => ModLoader::Forge,
                    "Fabric" => ModLoader::Fabric,
                    _ => ModLoader::NeoForge,
                };

                server_config.validate()?;
                config_manager.add_server(server_config);
                config_manager.save()?;

                println!(
                    "✓ Server '{}' added successfully!",
                    add_config.id.as_ref().unwrap()
                );
            }
        }
        Commands::Remove { target } => {
            if target.is_interactive() {
                println!("Starting interactive mode for removing server...");
                // TODO: Implement interactive server removal TUI
            } else if let Some(id) = &target.id {
                let mut config_manager = mc_link_config::ConfigManager::new()?;

                if !target.force {
                    print!("Are you sure you want to remove server '{id}'? (y/N): ");
                    use std::io::{self, Write};
                    io::stdout().flush().unwrap();
                    let mut input = String::new();
                    io::stdin().read_line(&mut input).unwrap();
                    if !input.trim().to_lowercase().starts_with('y') {
                        println!("Cancelled.");
                        return Ok(());
                    }
                }

                if let Some(removed) = config_manager.remove_server(id) {
                    config_manager.save()?;
                    println!("✓ Server '{}' removed successfully!", removed.name);
                } else {
                    return Err(format!("Server '{id}' not found.").into());
                }
            }
        }
        Commands::Scan { target } => {
            if target.is_interactive() {
                println!("Starting interactive mode for scanning server...");
                // TODO: Implement interactive server selection TUI
            } else if let Some(id) = &target.id {
                if let Some(server_config) = config.get_server(id) {
                    println!("Scanning server '{}'...", server_config.name);
                    let mut manager = MinecraftManager::from_config(server_config);

                    match manager.scan().await {
                        Ok(structure) => {
                            println!("✓ Scan complete!");
                            println!("Found {} mods on server.", structure.mods.mods.len());

                            if target.detailed {
                                println!("\nMod details:");
                                for (i, mod_info) in structure.mods.mods.iter().enumerate() {
                                    let version = mod_info.version.as_deref().unwrap_or("unknown");
                                    println!("  {}. {} ({})", i + 1, mod_info.name, version);
                                }
                            }
                        }
                        Err(e) => {
                            return Err(format!("Failed to scan server: {e}").into());
                        }
                    }
                } else {
                    return Err(format!("Server '{id}' not found.").into());
                }
            }
        }
        Commands::Compare { targets } => {
            if targets.is_interactive() {
                println!("Starting interactive mode for comparing servers...");
                // TODO: Implement interactive server selection TUI
            } else if let (Some(source), Some(target)) = (&targets.source, &targets.target) {
                let source_config = config
                    .get_server(source)
                    .ok_or(format!("Source server '{source}' not found."))?;
                let target_config = config
                    .get_server(target)
                    .ok_or(format!("Target server '{target}' not found."))?;

                println!(
                    "Comparing '{}' -> '{}'...",
                    source_config.name, target_config.name
                );

                let mut source_manager = MinecraftManager::from_config(source_config);
                let mut target_manager = MinecraftManager::from_config(target_config);

                use mc_link_manager::prelude::CompatConfig;
                let compat_config = CompatConfig::default();

                match source_manager
                    .compare_with(&mut target_manager, &compat_config)
                    .await
                {
                    Ok(plan) => {
                        println!("✓ Comparison complete!");
                        if targets.detailed {
                            println!("Sync plan: {plan:#?}");
                        } else {
                            println!("Use --detailed to see full comparison results.");
                        }
                    }
                    Err(e) => {
                        return Err(format!("Failed to compare servers: {e}").into());
                    }
                }
            }
        }
        Commands::Sync { targets } => {
            if targets.is_interactive() {
                println!("Starting interactive mode for syncing servers...");
                // TODO: Implement interactive server selection TUI
            } else if let (Some(source), Some(target)) = (&targets.source, &targets.target) {
                let source_config = config
                    .get_server(source)
                    .ok_or(format!("Source server '{source}' not found."))?;
                let target_config = config
                    .get_server(target)
                    .ok_or(format!("Target server '{target}' not found."))?;

                if targets.dry_run {
                    println!(
                        "DRY RUN: Would sync '{}' -> '{}'",
                        source_config.name, target_config.name
                    );
                } else {
                    println!(
                        "Syncing '{}' -> '{}'...",
                        source_config.name, target_config.name
                    );

                    if !targets.force {
                        print!("This will modify the target server. Continue? (y/N): ");
                        use std::io::{self, Write};
                        io::stdout().flush().unwrap();
                        let mut input = String::new();
                        io::stdin().read_line(&mut input).unwrap();
                        if !input.trim().to_lowercase().starts_with('y') {
                            println!("Cancelled.");
                            return Ok(());
                        }
                    }
                }

                let mut source_manager = MinecraftManager::from_config(source_config);
                let mut target_manager = MinecraftManager::from_config(target_config);

                use mc_link_manager::prelude::CompatConfig;
                let compat_config = CompatConfig::default();

                match source_manager
                    .compare_with(&mut target_manager, &compat_config)
                    .await
                {
                    Ok(plan) => {
                        if targets.dry_run {
                            println!("✓ Dry run complete! Sync plan:");
                            println!("{plan:#?}");
                        } else {
                            println!("✓ Sync plan generated!");
                            println!(
                                "Note: Actual sync execution would require additional implementation."
                            );
                            println!("Plan details: {plan:#?}");
                            // TODO: Implement actual sync execution
                            // This would involve:
                            // 1. Downloading missing mods from source
                            // 2. Uploading to target server
                            // 3. Removing mods that shouldn't be there
                            // 4. Handling version conflicts
                        }
                    }
                    Err(e) => {
                        return Err(format!("Failed to create sync plan: {e}").into());
                    }
                }
            }
        }
        Commands::Toggle { target } => {
            if target.is_interactive() {
                println!("Starting interactive mode for toggling server...");
                // TODO: Implement interactive server selection TUI
            } else if let Some(id) = &target.id {
                let mut config_manager = mc_link_config::ConfigManager::new()?;

                if let Some(server) = config_manager.servers_mut().servers.get_mut(id) {
                    if let Some(enabled) = target.enabled {
                        server.enabled = enabled;
                    } else {
                        server.enabled = !server.enabled;
                    }

                    let status = if server.enabled {
                        "enabled"
                    } else {
                        "disabled"
                    };
                    let name = server.name.clone();

                    config_manager.save()?;
                    println!("✓ Server '{name}' is now {status}.");
                } else {
                    return Err(format!("Server '{id}' not found.").into());
                }
            }
        }
        Commands::Config { id, json } => {
            if let Some(server_id) = id {
                if let Some(server) = config.get_server(&server_id) {
                    if json {
                        println!("{}", serde_json::to_string_pretty(server)?);
                    } else {
                        println!("Server: {}", server.name);
                        println!("ID: {}", server.id);
                        println!("Enabled: {}", server.enabled);
                        println!("Connection: {}", server.connection.type_name());
                        if let Some(mc_version) = &server.settings.minecraft_version {
                            println!("Minecraft Version: {mc_version}");
                        }
                        println!("Mod Loader: {:?}", server.settings.mod_loader);
                    }
                } else {
                    return Err(format!("Server '{server_id}' not found.").into());
                }
            } else {
                // Show all servers
                let servers = config.list_servers();
                if json {
                    println!("{}", serde_json::to_string_pretty(&servers)?);
                } else {
                    println!("Configuration Overview:");
                    println!("Config Directory: {}", config.config_dir().display());
                    println!("Servers: {}", servers.len());
                    for server_id in servers {
                        if let Some(server) = config.get_server(server_id) {
                            println!("  - {} [{}]", server.name, server.id);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

async fn run_tui() -> Result<(), Box<dyn std::error::Error>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run it
    let mut app = App::new()?;
    let res = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

async fn run_add_server_tui() -> Result<(), Box<dyn std::error::Error>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app in add server mode and run it
    let mut app = App::new()?;
    app.state = tui::app::AppState::AddServer;
    let res = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<()> {
    while app.running {
        terminal.draw(|f| app.render(f))?;
        if let Err(err) = app.handle_events().await {
            eprintln!("Error handling events: {err}");
            break;
        }
    }
    Ok(())
}
