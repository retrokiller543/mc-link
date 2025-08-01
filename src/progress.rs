//! CLI progress reporting using indicatif.

use indicatif::{ProgressBar, ProgressStyle};
use mc_link_core::{
    ProgressReporter, ProgressStage, ProgressUpdate, create_channel_progress_reporter,
    create_progress_sender,
};
use std::sync::{Arc, Mutex};

/// Creates a CLI progress reporter using indicatif progress bars with channel-based updates.
pub fn create_cli_progress_reporter() -> (ProgressReporter, tokio::task::JoinHandle<()>) {
    let (sender, mut receiver) = create_progress_sender();

    let progress_task = tokio::spawn(async move {
        let mut current_pb: Option<ProgressBar> = None;

        while let Some(update) = receiver.recv().await {
            // Create or update progress bar based on stage
            if current_pb.is_none() || should_create_new_bar(&update) {
                if let Some(ref pb) = current_pb {
                    pb.finish_and_clear();
                }
                current_pb = Some(create_progress_bar(&update));
            }

            if let Some(ref progress_bar) = current_pb {
                update_progress_bar(progress_bar, &update);

                // Finish progress bar if completed
                if update.is_complete() {
                    progress_bar.finish_with_message(format!("✓ {}", update.stage));
                    current_pb = None;
                }
            }
        }

        // Clean up any remaining progress bar
        if let Some(ref pb) = current_pb {
            pb.finish_and_clear();
        }
    });

    let reporter = create_channel_progress_reporter(sender);
    (reporter, progress_task)
}

/// Determines if we should create a new progress bar for this update.
fn should_create_new_bar(update: &ProgressUpdate) -> bool {
    matches!(
        update.stage,
        ProgressStage::Connecting
            | ProgressStage::Listing
            | ProgressStage::Downloading
            | ProgressStage::Analyzing
            | ProgressStage::Comparing
            | ProgressStage::Synchronizing
    )
}

/// Creates a new progress bar with appropriate styling for the stage.
fn create_progress_bar(update: &ProgressUpdate) -> ProgressBar {
    let pb = ProgressBar::new(update.total);

    let style = match update.stage {
        ProgressStage::Connecting => {
            ProgressStyle::with_template(
                "{spinner:.green} [{elapsed_precise}] {msg}"
            )
            .unwrap()
            .progress_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ")
        }
        ProgressStage::Downloading => {
            ProgressStyle::with_template(
                "{spinner:.blue} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} {msg} ({eta})"
            )
            .unwrap()
            .progress_chars("#>-")
        }
        ProgressStage::Analyzing => {
            ProgressStyle::with_template(
                "{spinner:.yellow} [{elapsed_precise}] [{wide_bar:.yellow/orange}] {pos}/{len} {msg} ({eta})"
            )
            .unwrap()
            .progress_chars("#>-")
        }
        ProgressStage::Comparing => {
            ProgressStyle::with_template(
                "{spinner:.magenta} [{elapsed_precise}] [{wide_bar:.magenta/pink}] {pos}/{len} {msg} ({eta})"
            )
            .unwrap()
            .progress_chars("#>-")
        }
        ProgressStage::Synchronizing => {
            ProgressStyle::with_template(
                "{spinner:.red} [{elapsed_precise}] [{wide_bar:.red/orange}] {pos}/{len} {msg} ({eta})"
            )
            .unwrap()
            .progress_chars("#>-")
        }
        _ => {
            ProgressStyle::with_template(
                "{spinner:.green} [{elapsed_precise}] [{wide_bar:.green/blue}] {pos}/{len} {msg} ({eta})"
            )
            .unwrap()
            .progress_chars("#>-")
        }
    };

    pb.set_style(style);
    pb.set_message(update.stage.to_string());
    pb
}

/// Updates the progress bar with new progress information.
fn update_progress_bar(pb: &ProgressBar, update: &ProgressUpdate) {
    pb.set_position(update.current);
    pb.set_length(update.total);

    let mut message = update.stage.to_string();
    if let Some(ref msg) = update.message {
        message = format!("{} - {}", update.stage, msg);
    }
    if let Some(throughput) = update.throughput {
        message = format!("{} ({}/s)", message, format_bytes(throughput));
    }

    pb.set_message(message);
}

/// Helper function to format bytes in a human-readable way.
fn format_bytes(bytes: u64) -> String {
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

/// Creates a simple CLI progress reporter for backwards compatibility
pub fn create_simple_cli_progress_reporter() -> ProgressReporter {
    let pb = Arc::new(Mutex::new(None::<ProgressBar>));

    Box::new(move |update: ProgressUpdate| {
        let mut pb_guard = pb.lock().unwrap();

        // Create or update progress bar based on stage
        if pb_guard.is_none() || should_create_new_bar(&update) {
            let new_pb = create_progress_bar(&update);
            *pb_guard = Some(new_pb);
        }

        if let Some(ref progress_bar) = *pb_guard {
            update_progress_bar(progress_bar, &update);

            // Finish progress bar if completed
            if update.is_complete() {
                progress_bar.finish_with_message(format!("✓ {}", update.stage));
                *pb_guard = None;
            }
        }
    })
}

/// Creates a simple spinner for indeterminate progress.
pub fn create_cli_spinner(message: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.green} {msg}")
            .unwrap()
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈"),
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(120));
    pb
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_link_core::ProgressStage;

    #[test]
    fn test_should_create_new_bar() {
        let update = ProgressUpdate::new(ProgressStage::Connecting, 0, 100);
        assert!(should_create_new_bar(&update));

        let update = ProgressUpdate::new(ProgressStage::CheckingCache, 50, 100);
        assert!(!should_create_new_bar(&update));

        let update = ProgressUpdate::new(ProgressStage::Downloading, 10, 100);
        assert!(should_create_new_bar(&update));
    }

    #[test]
    fn test_create_progress_bar() {
        let update = ProgressUpdate::new(ProgressStage::Downloading, 0, 100);
        let pb = create_progress_bar(&update);

        // Just verify it doesn't panic and creates a progress bar
        assert_eq!(pb.length().unwrap(), 100);
        assert_eq!(pb.position(), 0);
    }
}
