use crate::cli::commands::InitArgs;
use crate::config::{feature_flags::Feature, FuseConfig};
use crate::error::Result;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub fn handle(args: InitArgs) -> Result<()> {
    let config_path = FuseConfig::default_config_path();
    let fuse_dir = config_path.parent().unwrap();

    // Check if config already exists
    if config_path.exists() && !args.yes {
        print!(
            "Configuration already exists at {}. Overwrite? (y/N): ",
            config_path.display()
        );
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Initialization cancelled.");
            return Ok(());
        }
    }

    // Create .fuse directory
    std::fs::create_dir_all(fuse_dir)?;

    if let Some(source_file) = args.from_file {
        copy_from_file(source_file, fuse_dir, &config_path)?;
    } else if args.yes {
        create_default_config(&config_path)?;
    } else {
        interactive_setup(&config_path)?;
    }

    // Create example configs
    create_example_configs(fuse_dir)?;

    print_success_message(&config_path);

    Ok(())
}

fn copy_from_file(
    source_file: PathBuf,
    fuse_dir: &std::path::Path,
    config_path: &Path,
) -> Result<()> {
    println!("Copying configuration from: {}", source_file.display());

    let content = std::fs::read_to_string(&source_file)?;
    let target_path = if source_file.extension().and_then(|s| s.to_str()) == Some("yaml")
        || source_file.extension().and_then(|s| s.to_str()) == Some("yml")
    {
        fuse_dir.join("config.yaml")
    } else {
        config_path.to_path_buf()
    };

    std::fs::write(&target_path, content)?;

    // Validate the config
    if target_path.extension().and_then(|s| s.to_str()) == Some("yaml")
        || target_path.extension().and_then(|s| s.to_str()) == Some("yml")
    {
        FuseConfig::from_yaml_file(&target_path)?;
    } else {
        FuseConfig::from_toml_file(&target_path)?;
    }

    println!("✓ Configuration copied to: {}", target_path.display());
    Ok(())
}

fn create_default_config(config_path: &PathBuf) -> Result<()> {
    let config = FuseConfig::default();
    config.to_toml_file(config_path)?;
    println!(
        "✓ Configuration initialized with defaults at: {}",
        config_path.display()
    );
    Ok(())
}

fn interactive_setup(config_path: &PathBuf) -> Result<()> {
    println!("\n🚀 Fuse Configuration Setup\n");
    println!("Press Enter to use default values shown in [brackets]\n");

    let mut config = FuseConfig::default();

    // Models directory
    print!("Models directory [{}]: ", config.models_dir.display());
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    if !input.trim().is_empty() {
        config.models_dir = PathBuf::from(input.trim());
    }

    // Cache directory
    input.clear();
    print!("Cache directory [{}]: ", config.cache_dir.display());
    io::stdout().flush()?;
    io::stdin().read_line(&mut input)?;
    if !input.trim().is_empty() {
        config.cache_dir = PathBuf::from(input.trim());
    }

    // Log level
    input.clear();
    print!(
        "Log level (trace/debug/info/warn/error) [{}]: ",
        config.log_level
    );
    io::stdout().flush()?;
    io::stdin().read_line(&mut input)?;
    if !input.trim().is_empty() {
        config.log_level = input.trim().to_string();
    }

    // Server host
    input.clear();
    print!("Server host [{}]: ", config.server.host);
    io::stdout().flush()?;
    io::stdin().read_line(&mut input)?;
    if !input.trim().is_empty() {
        config.server.host = input.trim().to_string();
    }

    // Server port
    input.clear();
    print!("Server port [{}]: ", config.server.port);
    io::stdout().flush()?;
    io::stdin().read_line(&mut input)?;
    if !input.trim().is_empty() {
        if let Ok(port) = input.trim().parse() {
            config.server.port = port;
        }
    }

    println!("\n📦 Feature Flags (enable optional features)");
    println!("Enter 'y' to enable, or press Enter to skip\n");

    // Feature flags
    for feature in Feature::all() {
        input.clear();
        print!(
            "Enable {} - {}? (y/N): ",
            feature.name(),
            feature.description()
        );
        io::stdout().flush()?;
        io::stdin().read_line(&mut input)?;
        if input.trim().eq_ignore_ascii_case("y") {
            config.feature_flags.enable(feature);
        }
    }

    // Save configuration
    config.to_toml_file(config_path)?;

    println!("\n✓ Configuration saved to: {}", config_path.display());
    Ok(())
}

fn create_example_configs(fuse_dir: &std::path::Path) -> Result<()> {
    const TOML_EXAMPLE: &str = include_str!("../../../config.toml.example");
    const YAML_EXAMPLE: &str = include_str!("../../../config.yaml.example");

    std::fs::write(fuse_dir.join("config.toml.example"), TOML_EXAMPLE)?;
    std::fs::write(fuse_dir.join("config.yaml.example"), YAML_EXAMPLE)?;

    println!("✓ Example configurations copied to ~/.fuse/");
    Ok(())
}

fn print_success_message(config_path: &Path) {
    println!("\nYou can:");
    println!("  • View config: fuse config");
    println!("  • Edit config: vim {}", config_path.display());
    println!("  • View examples: cat ~/.fuse/config.toml.example");
    println!("  • Manage features: fuse features list");
}
