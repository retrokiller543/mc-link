use crate::tui::components::add_server::AddServerForm;
use crate::tui::events::{Event, EventHandler};
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    terminal::Frame,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

/// Application result type.
pub type AppResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Application state.
#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Menu,
    AddServer,
    ListServers,
    Success(String),
    Error(String),
}

/// Application.
pub struct App {
    /// Is the application running?
    pub running: bool,
    /// Current application state
    pub state: AppState,
    /// Menu state
    pub menu_state: ListState,
    /// Add server form
    pub add_server_form: AddServerForm,
    /// Event handler
    pub events: EventHandler,
}

impl App {
    /// Constructs a new instance of [`App`].
    pub fn new() -> AppResult<Self> {
        let mut menu_state = ListState::default();
        menu_state.select(Some(0));

        Ok(Self {
            running: true,
            state: AppState::Menu,
            menu_state,
            add_server_form: AddServerForm::new(),
            events: EventHandler::new(250),
        })
    }

    /// Handles the tick event of the terminal.
    pub fn tick(&self) {}

    /// Set running to false to quit the application.
    pub fn quit(&mut self) {
        self.running = false;
    }

    /// Handles key events and updates the state of [`App`].
    pub async fn handle_events(&mut self) -> AppResult<()> {
        match self
            .events
            .next()
            .await
            .map_err(|e| -> Box<dyn std::error::Error> { e })?
        {
            Event::Tick => self.tick(),
            Event::Key(key_event) => {
                match self.state {
                    AppState::Menu => self.handle_menu_keys(key_event),
                    AppState::AddServer => {
                        if let Some(result) = self.add_server_form.handle_key_event(key_event).await
                        {
                            match result {
                                Ok(_) => {
                                    // Server added successfully, show success message
                                    self.state = AppState::Success(
                                        "Server added successfully! Press any key to continue."
                                            .to_string(),
                                    );
                                    self.add_server_form = AddServerForm::new();
                                }
                                Err(e) => {
                                    // Show error message
                                    self.state = AppState::Error(format!(
                                        "Error: {e}. Press any key to continue."
                                    ));
                                }
                            }
                        }
                    }
                    AppState::Success(_) | AppState::Error(_) => {
                        // Any key returns to menu
                        self.state = AppState::Menu;
                    }
                    AppState::ListServers => self.handle_list_keys(key_event),
                }
            }
            Event::Mouse(_) => {}
            Event::Resize(_, _) => {}
        }
        Ok(())
    }

    fn handle_menu_keys(&mut self, key_event: crossterm::event::KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') | KeyCode::Esc => self.quit(),
            KeyCode::Char('c') if key_event.modifiers == KeyModifiers::CONTROL => self.quit(),
            KeyCode::Down | KeyCode::Char('j') => self.next_menu_item(),
            KeyCode::Up | KeyCode::Char('k') => self.previous_menu_item(),
            KeyCode::Enter => self.select_menu_item(),
            _ => {}
        }
    }

    fn handle_list_keys(&mut self, key_event: crossterm::event::KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') | KeyCode::Esc => self.state = AppState::Menu,
            KeyCode::Char('c') if key_event.modifiers == KeyModifiers::CONTROL => self.quit(),
            _ => {}
        }
    }

    fn next_menu_item(&mut self) {
        let i = match self.menu_state.selected() {
            Some(i) => {
                if i >= 4 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.menu_state.select(Some(i));
    }

    fn previous_menu_item(&mut self) {
        let i = match self.menu_state.selected() {
            Some(i) => {
                if i == 0 {
                    4
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.menu_state.select(Some(i));
    }

    fn select_menu_item(&mut self) {
        match self.menu_state.selected() {
            Some(0) => self.state = AppState::ListServers,
            Some(1) => self.state = AppState::AddServer,
            Some(2) => {
                // Scan server - could implement interactive selection
            }
            Some(3) => {
                // Sync servers - could implement interactive selection
            }
            Some(4) => self.quit(),
            _ => {}
        }
    }

    /// Renders the user interface widgets.
    pub fn render(&mut self, frame: &mut Frame) {
        match self.state.clone() {
            AppState::Menu => self.render_menu(frame),
            AppState::AddServer => self.render_add_server(frame),
            AppState::ListServers => self.render_list_servers(frame),
            AppState::Success(message) => self.render_message(frame, &message, Color::Green),
            AppState::Error(message) => self.render_message(frame, &message, Color::Red),
        }
    }

    fn render_menu(&mut self, frame: &mut Frame) {
        let area = frame.size();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
            ])
            .split(area);

        let title = Paragraph::new("MC-Link - Minecraft Server Management")
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

        let menu_items = vec![
            ListItem::new("📋 List Servers"),
            ListItem::new("➕ Add Server"),
            ListItem::new("🔍 Scan Server"),
            ListItem::new("🔄 Sync Servers"),
            ListItem::new("❌ Exit"),
        ];

        let menu = List::new(menu_items)
            .block(
                Block::default()
                    .title("Main Menu")
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::White)),
            )
            .style(Style::default().fg(Color::White))
            .highlight_style(
                Style::default()
                    .bg(Color::Yellow)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ");

        frame.render_stateful_widget(menu, chunks[1], &mut self.menu_state);

        let help = Paragraph::new("Use ↑↓ or j/k to navigate, Enter to select, q to quit")
            .style(Style::default().fg(Color::Gray))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::White)),
            );
        frame.render_widget(help, chunks[2]);
    }

    fn render_add_server(&mut self, frame: &mut Frame) {
        self.add_server_form.render(frame);
    }

    fn render_list_servers(&mut self, frame: &mut Frame) {
        let area = frame.size();

        let block = Block::default()
            .title("Server List")
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::White));

        let paragraph = Paragraph::new(
            "Server list will be implemented here.\n\nPress 'q' or Esc to return to menu.",
        )
        .block(block)
        .style(Style::default().fg(Color::White));

        frame.render_widget(paragraph, area);
    }

    fn render_message(&mut self, frame: &mut Frame, message: &str, color: Color) {
        let area = frame.size();

        let block = Block::default()
            .title("Message")
            .borders(Borders::ALL)
            .style(Style::default().fg(color));

        let paragraph = Paragraph::new(message)
            .block(block)
            .style(Style::default().fg(color));

        frame.render_widget(paragraph, area);
    }
}
