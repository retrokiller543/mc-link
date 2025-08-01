use crate::tui::form::{FieldType, FormField, FormState, ValidationResult};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mc_link_config::{ConfigManager, ConnectionType, FtpConnection, LocalConnection, ServerConfig};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    terminal::Frame,
    widgets::{Block, Borders, Paragraph},
};
use std::collections::HashMap;

/// Add server form component
pub struct AddServerForm {
    form_state: FormState,
}

impl AddServerForm {
    /// Create a new add server form
    pub fn new() -> Self {
        let fields = vec![
            FormField {
                name: "id".to_string(),
                label: "Server ID".to_string(),
                required: true,
                field_type: FieldType::Text,
                validation: Some(Box::new(|value| {
                    if value.trim().is_empty() {
                        return ValidationResult::Invalid("Server ID cannot be empty".to_string());
                    }
                    if !value
                        .chars()
                        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
                    {
                        return ValidationResult::Invalid(
                            "Server ID can only contain letters, numbers, hyphens, and underscores"
                                .to_string(),
                        );
                    }
                    ValidationResult::Valid
                })),
                help_text: Some(
                    "Unique identifier for this server (alphanumeric, -, _)".to_string(),
                ),
            },
            FormField {
                name: "name".to_string(),
                label: "Display Name".to_string(),
                required: true,
                field_type: FieldType::Text,
                validation: Some(Box::new(|value| {
                    if value.trim().is_empty() {
                        ValidationResult::Invalid("Display name cannot be empty".to_string())
                    } else {
                        ValidationResult::Valid
                    }
                })),
                help_text: Some("Human-readable name for this server".to_string()),
            },
            FormField {
                name: "connection_type".to_string(),
                label: "Connection Type".to_string(),
                required: true,
                field_type: FieldType::Select(()),
                validation: Some(Box::new(|value| match value.to_lowercase().as_str() {
                    "local" | "ftp" => ValidationResult::Valid,
                    _ => ValidationResult::Invalid(
                        "Connection type must be 'local' or 'ftp'".to_string(),
                    ),
                })),
                help_text: Some(
                    "Type 'local' for local filesystem or 'ftp' for FTP connection".to_string(),
                ),
            },
            FormField {
                name: "target".to_string(),
                label: "Target Path/Host".to_string(),
                required: true,
                field_type: FieldType::Text,
                validation: Some(Box::new(|value| {
                    if value.trim().is_empty() {
                        ValidationResult::Invalid("Target cannot be empty".to_string())
                    } else {
                        ValidationResult::Valid
                    }
                })),
                help_text: Some("For local: filesystem path, for FTP: host:port".to_string()),
            },
            FormField {
                name: "username".to_string(),
                label: "Username (FTP only)".to_string(),
                required: false,
                field_type: FieldType::Text,
                validation: None,
                help_text: Some("Username for FTP connection (leave empty for local)".to_string()),
            },
            FormField {
                name: "password".to_string(),
                label: "Password (FTP only)".to_string(),
                required: false,
                field_type: FieldType::Password,
                validation: None,
                help_text: Some("Password for FTP connection (leave empty for local)".to_string()),
            },
            FormField {
                name: "minecraft_version".to_string(),
                label: "Minecraft Version".to_string(),
                required: true,
                field_type: FieldType::Text,
                validation: Some(Box::new(|value| {
                    if value.trim().is_empty() {
                        ValidationResult::Invalid("Minecraft version cannot be empty".to_string())
                    } else {
                        ValidationResult::Valid
                    }
                })),
                help_text: Some("Minecraft version (e.g., 1.21.1)".to_string()),
            },
            FormField {
                name: "mod_loader".to_string(),
                label: "Mod Loader".to_string(),
                required: true,
                field_type: FieldType::Select(()),
                validation: Some(Box::new(|value| match value {
                    v if v == "NeoForge" || v == "Forge" || v == "Fabric" => {
                        ValidationResult::Valid
                    }
                    _ => ValidationResult::Invalid(
                        "Mod loader must be NeoForge, Forge, or Fabric".to_string(),
                    ),
                })),
                help_text: Some("Type 'NeoForge', 'Forge', or 'Fabric'".to_string()),
            },
        ];

        let mut form_state = FormState::new(fields);

        // Set default values
        form_state
            .values
            .insert("minecraft_version".to_string(), "1.21.1".to_string());
        form_state
            .values
            .insert("mod_loader".to_string(), "NeoForge".to_string());

        Self { form_state }
    }

    /// Handle key events
    pub async fn handle_key_event(
        &mut self,
        key_event: KeyEvent,
    ) -> Option<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
        match key_event.code {
            KeyCode::Esc => {
                // Cancel form
                return Some(Err("Form cancelled".into()));
            }
            KeyCode::Tab => {
                if key_event.modifiers.contains(KeyModifiers::SHIFT) {
                    self.form_state.previous_field();
                } else {
                    self.form_state.next_field();
                }
            }

            KeyCode::Enter if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                return match self.submit_form().await {
                    Ok(_) => Some(Ok(())),
                    Err(e) => Some(Err(e)),
                };
            }
            KeyCode::Enter => {
                self.form_state.next_field();
            }

            KeyCode::Char('s') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                // Alternative submit with Ctrl+S
                return match self.submit_form().await {
                    Ok(_) => Some(Ok(())),
                    Err(e) => Some(Err(e)),
                };
            }
            KeyCode::Char(c) => {
                // Don't handle chars if they have modifiers (except normal typing)
                if key_event.modifiers.is_empty() || key_event.modifiers == KeyModifiers::SHIFT {
                    self.form_state.handle_char(c);
                }
            }
            KeyCode::Backspace => {
                self.form_state.handle_backspace();
            }
            KeyCode::Up => {
                self.form_state.previous_field();
            }
            KeyCode::Down => {
                self.form_state.next_field();
            }
            _ => {
                // Ignore other keys
            }
        }
        None
    }

    /// Submit the form and create server config
    async fn submit_form(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match self.form_state.submit() {
            Ok(values) => {
                let server_config = self.create_server_config(values)?;

                // Add server to config manager
                let mut config_manager = ConfigManager::new()?;

                server_config
                    .validate()
                    .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;

                config_manager.add_server(server_config);
                config_manager.save()?;

                Ok(())
            }
            Err(_errors) => {
                // Form has validation errors, don't submit
                Err("Form has validation errors".into())
            }
        }
    }

    /// Create server config from form values
    fn create_server_config(
        &self,
        values: HashMap<String, String>,
    ) -> Result<ServerConfig, Box<dyn std::error::Error + Send + Sync>> {
        let connection_type = match values
            .get("connection_type")
            .unwrap()
            .to_lowercase()
            .as_str()
        {
            "local" => ConnectionType::Local(LocalConnection {
                path: values.get("target").unwrap().clone(),
            }),
            "ftp" => {
                let target = values.get("target").unwrap();
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
                    username: values.get("username").cloned().unwrap_or_default(),
                    password: values.get("password").cloned(),
                    ..Default::default()
                })
            }
            _ => return Err("Invalid connection type".into()),
        };

        let mut server_config = ServerConfig::new(
            values.get("id").unwrap().clone(),
            values.get("name").unwrap().clone(),
        );

        server_config.connection = connection_type;
        server_config.enabled = true;

        // Set server settings
        if let Some(mc_version) = values.get("minecraft_version") {
            server_config.settings.minecraft_version = Some(mc_version.clone());
        }

        // Parse mod loader
        if let Some(mod_loader) = values.get("mod_loader") {
            use mc_link_config::ModLoader;
            server_config.settings.mod_loader = match mod_loader.as_str() {
                "NeoForge" => ModLoader::NeoForge,
                "Forge" => ModLoader::Forge,
                "Fabric" => ModLoader::Fabric,
                _ => ModLoader::NeoForge,
            };
        }

        Ok(server_config)
    }

    /// Render the form
    pub fn render(&mut self, frame: &mut Frame) {
        let area = frame.size();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(area);

        // Title
        let title = Paragraph::new("Add New Server")
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::White)),
            );
        frame.render_widget(title, chunks[0]);

        // Form
        self.form_state.render(frame, chunks[1]);
    }
}
