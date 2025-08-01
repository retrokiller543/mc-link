/// Generic form builder for serde-compatible structs
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    terminal::Frame,
    widgets::{Block, Borders, Paragraph},
};
use std::collections::HashMap;

/// Validation result for form fields
#[derive(Debug, Clone)]
pub enum ValidationResult {
    Valid,
    Invalid(String),
}

/// Type alias for validation function
type ValidationFn = Box<dyn Fn(&str) -> ValidationResult + Send + Sync>;

/// Form field definition
pub struct FormField {
    pub name: String,
    pub label: String,
    pub required: bool,
    pub field_type: FieldType,
    pub validation: Option<ValidationFn>,
    pub help_text: Option<String>,
}

/// Types of form fields
#[derive(Debug, Clone)]
pub enum FieldType {
    Text,
    Select(()),
    Password,
}

/// Generic form state
pub struct FormState {
    pub fields: Vec<FormField>,
    pub values: HashMap<String, String>,
    pub current_field: usize,
    pub errors: HashMap<String, String>,
    pub submitted: bool,
}

impl FormState {
    /// Create a new form state
    pub fn new(fields: Vec<FormField>) -> Self {
        let mut values = HashMap::new();

        for field in &fields {
            values.insert(field.name.clone(), String::new());
        }

        Self {
            fields,
            values,
            current_field: 0,
            errors: HashMap::new(),
            submitted: false,
        }
    }

    /// Move to next field
    pub fn next_field(&mut self) {
        if self.current_field < self.fields.len() - 1 {
            self.current_field += 1;
        }
    }

    /// Move to previous field
    pub fn previous_field(&mut self) {
        if self.current_field > 0 {
            self.current_field -= 1;
        }
    }

    /// Get current field name
    pub fn current_field_name(&self) -> Option<&str> {
        self.fields.get(self.current_field).map(|f| f.name.as_str())
    }

    /// Handle character input for current field
    pub fn handle_char(&mut self, c: char) {
        if let Some(field_name) = self.current_field_name() {
            let field_name = field_name.to_string();
            if let Some(value) = self.values.get_mut(&field_name) {
                value.push(c);

                // Clear error for this field when user starts typing
                self.errors.remove(&field_name);
            }
        }
    }

    /// Handle backspace for current field
    pub fn handle_backspace(&mut self) {
        if let Some(field_name) = self.current_field_name() {
            let field_name = field_name.to_string();
            if let Some(value) = self.values.get_mut(&field_name) {
                value.pop();

                // Clear error for this field when user starts typing
                self.errors.remove(&field_name);
            }
        }
    }

    /// Validate all fields
    pub fn validate(&mut self) -> bool {
        self.errors.clear();
        let mut valid = true;

        for field in &self.fields {
            let empty_string = String::new();
            let value = self.values.get(&field.name).unwrap_or(&empty_string);

            // Check required fields
            if field.required && value.trim().is_empty() {
                self.errors
                    .insert(field.name.clone(), format!("{} is required", field.label));
                valid = false;
                continue;
            }

            // Run custom validation if present
            if let Some(validator) = &field.validation {
                match validator(value) {
                    ValidationResult::Valid => {}
                    ValidationResult::Invalid(error) => {
                        self.errors.insert(field.name.clone(), error);
                        valid = false;
                    }
                }
            }
        }

        valid
    }

    /// Submit the form
    pub fn submit(&mut self) -> Result<HashMap<String, String>, HashMap<String, String>> {
        self.submitted = true;
        if self.validate() {
            Ok(self.values.clone())
        } else {
            Err(self.errors.clone())
        }
    }

    /// Render the form
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                self.fields
                    .iter()
                    .map(|_| Constraint::Length(3))
                    .chain(std::iter::once(Constraint::Min(1)))
                    .collect::<Vec<_>>(),
            )
            .split(area);

        for (i, field) in self.fields.iter().enumerate() {
            let is_focused = i == self.current_field;
            let has_error = self.errors.contains_key(&field.name);

            let mut block = Block::default()
                .borders(Borders::ALL)
                .title(field.label.as_str());

            if is_focused {
                block = block.border_style(Style::default().fg(Color::Yellow));
            } else if has_error {
                block = block.border_style(Style::default().fg(Color::Red));
            }

            let empty_string = String::new();
            let value = self.values.get(&field.name).unwrap_or(&empty_string);
            let display_value = if matches!(field.field_type, FieldType::Password) {
                "*".repeat(value.len())
            } else {
                value.clone()
            };

            // Add cursor indicator for focused field
            let display_text = if is_focused {
                format!("{display_value}█")
            } else {
                display_value
            };

            let input_widget =
                Paragraph::new(display_text.as_str())
                    .block(block)
                    .style(if is_focused {
                        Style::default().fg(Color::White)
                    } else {
                        Style::default().fg(Color::Gray)
                    });

            frame.render_widget(input_widget, chunks[i]);

            // Show error if present
            if let Some(error) = self.errors.get(&field.name) {
                let error_area = Rect {
                    x: chunks[i].x + 1,
                    y: chunks[i].y + chunks[i].height - 1,
                    width: chunks[i].width - 2,
                    height: 1,
                };
                let error_text =
                    Paragraph::new(error.as_str()).style(Style::default().fg(Color::Red));
                frame.render_widget(error_text, error_area);
            }
        }

        // Show help text at bottom
        if let Some(help_area) = chunks.last() {
            let help_text = if let Some(current_field) = self.fields.get(self.current_field) {
                current_field.help_text.as_deref().unwrap_or("Tab/↑↓ to navigate, Enter=next field, Ctrl+Enter/Ctrl+S to submit, Esc to cancel")
            } else {
                "Tab/↑↓ to navigate, Enter=next field, Ctrl+Enter/Ctrl+S to submit, Esc to cancel"
            };

            let help = Paragraph::new(help_text)
                .style(Style::default().fg(Color::Gray))
                .block(Block::default().borders(Borders::ALL).title("Help"));
            frame.render_widget(help, *help_area);
        }
    }
}
