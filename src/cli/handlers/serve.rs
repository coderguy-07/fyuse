//! Handler for the `fuse serve` command — starts the API server.

use crate::api::server::{ApiConfig, ApiServer, ApiState};
use crate::cli::commands::ServeArgs;
use crate::config::FuseConfig;
use crate::error::Result;
use std::net::SocketAddr;

/// Handle the serve command — start the API server.
pub async fn handle_serve(args: ServeArgs, config: &FuseConfig) -> Result<()> {
    let host = args.host.as_deref().unwrap_or(&config.server.host);
    let port = args.port.unwrap_or(config.server.port);

    let api_config = ApiConfig {
        host: host.to_string(),
        port,
        cors_origins: vec!["*".to_string()],
    };

    let state = ApiState {
        models_dir: config.models_dir.clone(),
        config: api_config,
    };

    let server = ApiServer::new(state);
    let addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .map_err(|e| crate::error::FuseError::InternalError(format!("Invalid address: {}", e)))?;

    println!("Fuse API server starting on http://{}", addr);
    println!("  Ollama API:    http://{}/api/generate", addr);
    println!("  OpenAI API:    http://{}/v1/chat/completions", addr);
    println!("  Anthropic API: http://{}/v1/messages", addr);
    println!("  Health:        http://{}/health", addr);
    println!();
    println!("Press Ctrl+C to stop.");

    server.serve(addr).await
}

/// Run system diagnostics and report health.
pub async fn handle_doctor(config: &FuseConfig) -> Result<()> {
    println!("Fuse Doctor — System Health Check\n");

    let mut issues = 0;
    let mut warnings = 0;

    // 1. Check config
    print!("  Configuration... ");
    match config.validate() {
        Ok(()) => println!("OK"),
        Err(e) => {
            println!("FAIL: {}", e);
            issues += 1;
        }
    }

    // 2. Check models directory
    print!("  Models directory ({})... ", config.models_dir.display());
    if config.models_dir.exists() {
        let count = std::fs::read_dir(&config.models_dir)
            .map(|entries| entries.count())
            .unwrap_or(0);
        println!("OK ({} entries)", count);
    } else {
        println!("MISSING (will be created on first pull)");
        warnings += 1;
    }

    // 3. Check data directory
    print!("  Data directory ({})... ", config.data_dir.display());
    if config.data_dir.exists() {
        println!("OK");
    } else {
        println!("MISSING (will be created on first use)");
        warnings += 1;
    }

    // 4. Check system resources
    print!("  System resources... ");
    let sys = sysinfo::System::new_all();
    let total_ram = sys.total_memory();
    let available_ram = sys.available_memory();
    let cpus = sys.cpus().len();
    println!(
        "OK ({} CPUs, {:.1} GB RAM, {:.1} GB available)",
        cpus,
        total_ram as f64 / 1024.0 / 1024.0 / 1024.0,
        available_ram as f64 / 1024.0 / 1024.0 / 1024.0,
    );

    // 5. Check if server port is available
    print!(
        "  Server port ({}:{})... ",
        config.server.host, config.server.port
    );
    match std::net::TcpListener::bind(format!("{}:{}", config.server.host, config.server.port)) {
        Ok(_) => println!("OK (available)"),
        Err(_) => {
            println!("IN USE");
            warnings += 1;
        }
    }

    // 6. Check Rust/cargo features
    print!("  CPU inference feature... ");
    #[cfg(feature = "cpu-inference")]
    println!("ENABLED");
    #[cfg(not(feature = "cpu-inference"))]
    {
        println!("DISABLED");
        warnings += 1;
    }

    // Summary
    println!();
    if issues == 0 && warnings == 0 {
        println!("All checks passed. Fuse is ready to go!");
    } else if issues == 0 {
        println!("No critical issues found. {} warning(s) noted.", warnings);
    } else {
        println!(
            "{} issue(s) and {} warning(s) found. Please fix before running.",
            issues, warnings
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serve_args_defaults() {
        let args = ServeArgs {
            port: None,
            host: None,
            ollama_api: true,
            openai_api: true,
            anthropic_api: true,
        };
        assert!(args.port.is_none());
        assert!(args.host.is_none());
        assert!(args.ollama_api);
    }

    #[test]
    fn test_serve_args_custom() {
        let args = ServeArgs {
            port: Some(8080),
            host: Some("0.0.0.0".to_string()),
            ollama_api: true,
            openai_api: false,
            anthropic_api: false,
        };
        assert_eq!(args.port, Some(8080));
        assert_eq!(args.host.as_deref(), Some("0.0.0.0"));
    }

    #[tokio::test]
    async fn test_doctor_runs_without_panic() {
        let config = FuseConfig::default();
        let result = handle_doctor(&config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_doctor_with_invalid_config() {
        let mut config = FuseConfig::default();
        config.log_level = "invalid_level".to_string();
        // Doctor should still complete even with config issues
        let result = handle_doctor(&config).await;
        assert!(result.is_ok());
    }
}
