use crate::Result;
use chrono::Local;
use mc_link_config::{LogFileNameFormat, ManagerConfig};
use std::path::Path;
use tosic_utils::logging::{FilterConfig, TracingSubscriberBuilder};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::fmt::layer;

fn get_log_filename(format: &LogFileNameFormat) -> String {
    match format {
        LogFileNameFormat::Date => format!("mc-link-{}.log", Local::now().format("%Y-%m-%d")),
        LogFileNameFormat::Timestamp => format!("mc-link-{}.log", Local::now().timestamp()),
        LogFileNameFormat::DateTime => {
            format!("mc-link-{}.log", Local::now().format("%Y-%m-%d_%H-%M-%S"))
        }
        LogFileNameFormat::None => "mc-link.log".to_string(),
    }
}

/// Creates an environment-based filter for tracing output.
///
/// The filter respects the `RUST_LOG` environment variable and uses
/// default filtering configuration.
///
/// # Returns
///
/// ```ignore
fn tracing_env_filter() -> EnvFilter {
    FilterConfig::default().use_env(true).build()
}

/// Initializes the global tracing subscriber with configured layers and filters.
///
/// Sets up a non-blocking tracing subscriber that outputs to stdout with:
/// - Environment-based filtering (respects `RUST_LOG`)
/// - Compact formatting with thread names and line numbers
/// - Span event tracking
/// - Non-blocking I/O to prevent log contention
///
/// # Returns
///
/// Returns [`Result<Vec<WorkerGuard>>`] which is:
/// * `Ok(Vec<WorkerGuard>)` - Guard objects that must be kept alive for logging to work
/// * `Err(Error)` - If subscriber initialization fails
///
/// # Errors
///
/// This function will return an error if:
/// * The tracing subscriber is already initialized
/// * The non-blocking writer setup fails
/// * Filter configuration is invalid
///
/// # Examples
///
/// ```ignore
/// use nexsockd::tracing;
///
/// let _guards = tracing().expect("Failed to initialize tracing");
/// // Keep guards alive for the duration of the program
/// ```
///
/// Initializes the global tracing subscriber with non-blocking stdout logging.
///
/// Configures tracing to output logs to stdout with compact formatting, thread names, line numbers, log levels, and span close event tracking. Applies environment-based filtering. Returns a vector of `WorkerGuard` objects that must be kept alive to ensure logging remains active.
///
/// # Returns
/// A vector of `WorkerGuard` objects for maintaining the logging output.
///
/// # Errors
/// Returns an error if the tracing subscriber is already initialized or if logging setup fails.
///
/// # Examples
///
/// ```ignore
/// # use nexsockd::tracing;
/// let guards = tracing().expect("Failed to initialize tracing");
/// // Keep `guards` alive for the duration of the application.
/// ```
pub fn tracing(log_dir: &Path, config: &ManagerConfig) -> Result<Vec<WorkerGuard>> {
    let file_appender = rolling::daily(log_dir, get_log_filename(&config.log_file));
    let (file_writer, file_guard) = tracing_appender::non_blocking(file_appender);

    let mut guards = vec![file_guard];

    if config.log_to_stdout {
        let (stdout_writer, stdout_guard) = tracing_appender::non_blocking(std::io::stdout());
        guards.push(stdout_guard);

        TracingSubscriberBuilder::new()
            .with_filter(tracing_env_filter())
            .with_layer(
                layer()
                    .with_writer(stdout_writer)
                    .with_file(false)
                    .with_thread_names(true)
                    //.with_thread_ids(true)
                    .with_line_number(true)
                    .with_level(true)
                    .with_span_events(FmtSpan::CLOSE)
                    .compact(),
            )
            .with_layer(
                layer()
                    .with_writer(file_writer)
                    .with_file(true)
                    .with_thread_names(true)
                    //.with_thread_ids(true)
                    .with_line_number(true)
                    .with_level(true)
                    .with_span_events(FmtSpan::CLOSE)
                    .compact(),
            )
            .init()
            .map_err(Into::into)
            .map(|mut tracing_guards| {
                tracing_guards.extend(guards);
                tracing_guards
            })
    } else {
        TracingSubscriberBuilder::new()
            .with_filter(tracing_env_filter())
            .with_layer(
                layer()
                    .with_writer(file_writer)
                    .with_file(true)
                    .with_thread_names(true)
                    //.with_thread_ids(true)
                    .with_line_number(true)
                    .with_level(true)
                    .with_span_events(FmtSpan::CLOSE)
                    .compact(),
            )
            .init()
            .map_err(Into::into)
            .map(|mut tracing_guards| {
                tracing_guards.extend(guards);
                tracing_guards
            })
    }
}
