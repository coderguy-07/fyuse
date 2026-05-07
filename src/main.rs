use clap::Parser;
use fuse::{Cli, FeatureFlagManager, FuseConfig};

#[tokio::main]
async fn main() {
    // Print the logo
    Cli::print_logo();

    // Parse CLI arguments
    let cli = Cli::parse();

    // Load configuration
    let config = load_configuration(&cli);

    // Initialize logging
    initialize_logging(&cli, &config);

    // Create feature flag manager
    let feature_manager = FeatureFlagManager::new(config.feature_flags.clone());

    // Execute command
    if let Err(e) = fuse::cli::handlers::handle_command(cli.command, config, feature_manager).await
    {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn load_configuration(cli: &Cli) -> FuseConfig {
    if let Some(config_path) = &cli.config {
        FuseConfig::from_file(config_path).unwrap_or_else(|e| {
            eprintln!("Failed to load configuration: {}", e);
            std::process::exit(1);
        })
    } else {
        FuseConfig::load_or_default().unwrap_or_else(|e| {
            eprintln!("Failed to load configuration: {}", e);
            std::process::exit(1);
        })
    }
}

fn initialize_logging(cli: &Cli, config: &FuseConfig) {
    let log_level = cli.log_level.as_ref().unwrap_or(&config.log_level);

    // Use DirectoryManager for logs directory
    let log_dir = if let Ok(dir_manager) = fuse::config::DirectoryManager::new() {
        Some(dir_manager.logs_dir())
    } else {
        dirs::home_dir().map(|h| h.join(".fuse_cli").join("logs"))
    };

    if let Err(e) = fuse::logging::init_logging(log_level, log_dir) {
        eprintln!("Failed to initialize logging: {}", e);
        std::process::exit(1);
    }
}
