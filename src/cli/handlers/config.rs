use crate::cli::commands::ConfigArgs;
use crate::config::FuseConfig;
use crate::error::Result;

pub fn handle(args: ConfigArgs, config: &FuseConfig) -> Result<()> {
    if args.path {
        show_config_path();
    } else if args.validate {
        validate_config(config)?;
    } else {
        show_config(config)?;
    }

    Ok(())
}

fn show_config_path() {
    println!(
        "Configuration file: {}",
        FuseConfig::default_config_path().display()
    );
}

fn validate_config(config: &FuseConfig) -> Result<()> {
    config.validate()?;
    println!("✓ Configuration is valid");
    Ok(())
}

fn show_config(config: &FuseConfig) -> Result<()> {
    println!("Current configuration:");
    println!("{}", toml::to_string_pretty(config)?);
    Ok(())
}
