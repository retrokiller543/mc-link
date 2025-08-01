//! Progress reporting utilities for long-running operations.

use std::fmt;
use tokio::sync::mpsc;

/// Represents different stages of a Minecraft server operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgressStage {
    /// Establishing connection to server
    Connecting,
    /// Listing files on the server
    Listing,
    /// Downloading files from server
    Downloading,
    /// Analyzing downloaded files (JAR extraction, etc.)
    Analyzing,
    /// Comparing server structures
    Comparing,
    /// Synchronizing files between servers
    Synchronizing,
    /// Checking cache for existing data
    CheckingCache,
    /// Updating cache with new data
    UpdatingCache,
    /// Cleaning up temporary files
    CleaningUp,
    /// Operation completed
    Completed,
}

impl fmt::Display for ProgressStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProgressStage::Connecting => write!(f, "Connecting"),
            ProgressStage::Listing => write!(f, "Listing files"),
            ProgressStage::Downloading => write!(f, "Downloading"),
            ProgressStage::Analyzing => write!(f, "Analyzing"),
            ProgressStage::Comparing => write!(f, "Comparing"),
            ProgressStage::Synchronizing => write!(f, "Synchronizing"),
            ProgressStage::CheckingCache => write!(f, "Checking cache"),
            ProgressStage::UpdatingCache => write!(f, "Updating cache"),
            ProgressStage::CleaningUp => write!(f, "Cleaning up"),
            ProgressStage::Completed => write!(f, "Completed"),
        }
    }
}

/// Represents a progress update for an operation.
#[derive(Debug, Clone)]
pub struct ProgressUpdate {
    /// Current stage of the operation
    pub stage: ProgressStage,
    /// Current progress value (e.g., files processed)
    pub current: u64,
    /// Total expected progress (e.g., total files)
    pub total: u64,
    /// Optional message providing additional context
    pub message: Option<String>,
    /// Optional indication of data transfer rate (bytes per second)
    pub throughput: Option<u64>,
}

impl ProgressUpdate {
    /// Creates a new progress update.
    pub fn new(stage: ProgressStage, current: u64, total: u64) -> Self {
        Self {
            stage,
            current,
            total,
            message: None,
            throughput: None,
        }
    }

    /// Creates a progress update with a message.
    pub fn with_message(stage: ProgressStage, current: u64, total: u64, message: String) -> Self {
        Self {
            stage,
            current,
            total,
            message: Some(message),
            throughput: None,
        }
    }

    /// Creates a progress update with throughput information.
    pub fn with_throughput(
        stage: ProgressStage,
        current: u64,
        total: u64,
        throughput: u64,
    ) -> Self {
        Self {
            stage,
            current,
            total,
            message: None,
            throughput: Some(throughput),
        }
    }

    /// Returns the progress as a percentage (0.0-1.0).
    pub fn percentage(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.current as f64 / self.total as f64
        }
    }

    /// Returns whether the operation is complete.
    pub fn is_complete(&self) -> bool {
        self.stage == ProgressStage::Completed || self.current >= self.total
    }
}

impl fmt::Display for ProgressUpdate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {}/{} ({:.1}%)",
            self.stage,
            self.current,
            self.total,
            self.percentage() * 100.0
        )?;

        if let Some(ref message) = self.message {
            write!(f, " - {}", message)?;
        }

        if let Some(throughput) = self.throughput {
            write!(f, " ({}/s)", format_bytes(throughput))?;
        }

        Ok(())
    }
}

/// Type alias for progress update senders
pub type ProgressSender = mpsc::UnboundedSender<ProgressUpdate>;

/// Enhanced progress callback that receives structured progress updates.
pub type ProgressReporter = Box<dyn Fn(ProgressUpdate) + Send + Sync>;

/// Trait for operations that can report progress.
pub trait ProgressAware {
    /// Sets the progress reporter for this operation.
    fn set_progress_reporter(&mut self, reporter: ProgressReporter);

    /// Removes the progress reporter.
    fn clear_progress_reporter(&mut self);
}

/// Helper function to format bytes in a human-readable way.
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

/// Creates a progress sender that can be cloned and used across threads
pub fn create_progress_sender() -> (ProgressSender, mpsc::UnboundedReceiver<ProgressUpdate>) {
    mpsc::unbounded_channel()
}

/// Creates a progress reporter that sends updates via a channel
pub fn create_channel_progress_reporter(sender: ProgressSender) -> ProgressReporter {
    Box::new(move |update: ProgressUpdate| {
        let _ = sender.send(update);
    })
}

/// Helper function to create a simple progress reporter that reports to a callback.
pub fn create_progress_reporter<F>(callback: F) -> ProgressReporter
where
    F: Fn(ProgressUpdate) + Send + Sync + 'static,
{
    Box::new(callback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_percentage() {
        let progress = ProgressUpdate::new(ProgressStage::Downloading, 50, 100);
        assert_eq!(progress.percentage(), 0.5);

        let progress = ProgressUpdate::new(ProgressStage::Downloading, 0, 100);
        assert_eq!(progress.percentage(), 0.0);

        let progress = ProgressUpdate::new(ProgressStage::Downloading, 100, 100);
        assert_eq!(progress.percentage(), 1.0);

        let progress = ProgressUpdate::new(ProgressStage::Downloading, 0, 0);
        assert_eq!(progress.percentage(), 0.0);
    }

    #[test]
    fn test_progress_is_complete() {
        let progress = ProgressUpdate::new(ProgressStage::Completed, 50, 100);
        assert!(progress.is_complete());

        let progress = ProgressUpdate::new(ProgressStage::Downloading, 100, 100);
        assert!(progress.is_complete());

        let progress = ProgressUpdate::new(ProgressStage::Downloading, 50, 100);
        assert!(!progress.is_complete());
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.0 GB");
    }

    #[test]
    fn test_progress_display() {
        let progress = ProgressUpdate::new(ProgressStage::Downloading, 50, 100);
        assert_eq!(progress.to_string(), "Downloading: 50/100 (50.0%)");

        let progress = ProgressUpdate::with_message(
            ProgressStage::Downloading,
            25,
            100,
            "mod-file.jar".to_string(),
        );
        assert_eq!(
            progress.to_string(),
            "Downloading: 25/100 (25.0%) - mod-file.jar"
        );

        let progress = ProgressUpdate::with_throughput(ProgressStage::Downloading, 50, 100, 1024);
        assert_eq!(
            progress.to_string(),
            "Downloading: 50/100 (50.0%) (1.0 KB/s)"
        );
    }
}
