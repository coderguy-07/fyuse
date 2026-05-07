use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

/// Initialize logging infrastructure
pub fn init_logging(
    log_level: &str,
    log_dir: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create env filter from log level
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));

    // Console layer with formatting
    let console_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_file(false)
        .with_line_number(false)
        .compact()
        .with_filter(env_filter.clone());

    // Build subscriber with console layer
    let subscriber = tracing_subscriber::registry().with(console_layer);

    // Add file layer if log directory is specified
    if let Some(log_path) = log_dir {
        // Create log directory if it doesn't exist
        std::fs::create_dir_all(&log_path)?;

        let log_file = log_path.join(format!(
            "fuse-{}.log",
            chrono::Local::now().format("%Y%m%d")
        ));
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file)?;

        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(file)
            .with_ansi(false)
            .json()
            .with_filter(env_filter);

        subscriber.with(file_layer).init();
    } else {
        subscriber.init();
    }

    tracing::info!("Logging initialized with level: {}", log_level);

    Ok(())
}

/// Log context for operations
#[derive(Debug, Clone)]
pub struct LogContext {
    pub operation: String,
    pub model_name: Option<String>,
    pub user_id: Option<String>,
}

impl LogContext {
    pub fn new(operation: impl Into<String>) -> Self {
        Self {
            operation: operation.into(),
            model_name: None,
            user_id: None,
        }
    }

    pub fn with_model(mut self, model_name: impl Into<String>) -> Self {
        self.model_name = Some(model_name.into());
        self
    }

    pub fn with_user(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }
}

/// Macro for logging with context
#[macro_export]
macro_rules! log_with_context {
    ($level:ident, $ctx:expr, $($arg:tt)*) => {
        tracing::$level!(
            operation = %$ctx.operation,
            model_name = ?$ctx.model_name,
            user_id = ?$ctx.user_id,
            $($arg)*
        )
    };
}
