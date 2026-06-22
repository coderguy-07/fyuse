use crate::cli::commands::*;
use crate::cli::validation;
use crate::config::feature_flags::FeatureFlagManager;
use crate::config::FuseConfig;
use crate::error::Result;

pub async fn handle_pull(
    args: PullArgs,
    config: &FuseConfig,
    _feature_manager: &FeatureFlagManager,
) -> Result<()> {
    use crate::model::{Auth, ModelManager, ModelSource, Provider};
    use crate::storage::{Database, ModelRepository};
    use std::sync::Arc;
    use tracing::info;

    validation::validate_model_name(&args.model)?;

    let (repo, version_tag) = parse_model_ref(&args.model);
    let provider = resolve_provider(&args.source, &repo)?;
    let model_name = derive_local_name(&repo);

    let source = build_model_source(provider, &repo, version_tag);

    let db_path = config.models_dir.join("fuse.redb");
    let db = Arc::new(Database::new(db_path)?);
    let repository = Arc::new(ModelRepository::new(db));
    let manager = ModelManager::new(repository, config.models_dir.clone());

    let auth = resolve_auth_from_env();

    println!("Pulling {} from {}…", model_name, source);
    if let Some(ref fmt) = args.format {
        println!("Format requested: {}", fmt);
    }
    if args.resume {
        println!("Resume mode enabled");
    }
    info!(model = %model_name, source = %source, format = ?args.format, resume = args.resume, "Starting pull");

    let metadata = manager.pull(source, &model_name, auth, args.format, args.resume).await?;

    println!(
        "✓ {} pulled successfully ({}, {})",
        metadata.name,
        metadata.size_human_readable(),
        metadata.version
    );

    Ok(())
}

/// Split "org/model:v1.2" into ("org/model", Some("v1.2")).
fn parse_model_ref(model_ref: &str) -> (String, Option<String>) {
    match model_ref.split_once(':') {
        Some((repo, version)) => (repo.to_string(), Some(version.to_string())),
        None => (model_ref.to_string(), None),
    }
}

/// Derive a filesystem-safe local name from the repo path.
/// "meta-llama/Llama-3.2-1B" → "Llama-3.2-1B"
fn derive_local_name(repo: &str) -> String {
    repo.split('/')
        .last()
        .unwrap_or(repo)
        .to_string()
}

/// Resolve the provider from the --source flag or auto-detect from the model name.
fn resolve_provider(source_flag: &Option<String>, repo: &str) -> Result<crate::model::Provider> {
    use crate::model::Provider;
    match source_flag {
        Some(s) => s
            .parse::<Provider>()
            .map_err(crate::error::FuseError::ValidationError),
        None => {
            if repo.starts_with("unsloth/") {
                Ok(Provider::Unsloth)
            } else {
                Ok(Provider::HuggingFace)
            }
        }
    }
}

/// Construct a ModelSource from the resolved provider and optional version tag.
fn build_model_source(
    provider: crate::model::Provider,
    repo: &str,
    version: Option<String>,
) -> crate::model::ModelSource {
    use crate::model::{ModelSource, Provider};
    let source = match provider {
        Provider::HuggingFace => ModelSource::huggingface(repo),
        Provider::Unsloth => ModelSource::unsloth(repo),
        Provider::Ollama => ModelSource::ollama(repo),
        Provider::ModelScope => ModelSource::modelscope(repo),
        Provider::Remote => ModelSource::remote(repo),
        Provider::Local => ModelSource::local(repo),
    };
    match version {
        Some(v) => source.with_version(v),
        None => source,
    }
}

/// Read HuggingFace token from environment.
fn resolve_auth_from_env() -> Option<crate::model::Auth> {
    std::env::var("HF_TOKEN")
        .or_else(|_| std::env::var("HUGGING_FACE_HUB_TOKEN"))
        .ok()
        .map(crate::model::Auth::ApiKey)
}

pub async fn handle_run(
    args: RunArgs,
    config: &FuseConfig,
    _feature_manager: &FeatureFlagManager,
) -> Result<()> {
    use crate::server::start_server;

    // Create a modified config with the specified port if provided
    let mut server_config = config.clone();
    if let Some(port) = args.port {
        server_config.server.port = port;
    }

    println!("Starting Fuse server for model: {}", args.model);
    println!(
        "Server will be available at http://{}:{}",
        server_config.server.host, server_config.server.port
    );

    // Start the server
    start_server(server_config).await?;

    Ok(())
}

pub async fn handle_rm(args: RmArgs, config: &FuseConfig) -> Result<()> {
    use crate::model::ModelManager;
    use crate::storage::Database;
    use crate::storage::ModelRepository;
    use std::sync::Arc;

    // Initialize database and repository
    let db_path = config.models_dir.join("fuse.redb");
    let db = Arc::new(Database::new(db_path)?);
    let repository = Arc::new(ModelRepository::new(db));

    // Create model manager
    let manager = ModelManager::new(repository, config.models_dir.clone());

    // Check if model exists
    if manager.get_metadata(&args.model).await?.is_none() {
        return Err(crate::error::FuseError::ModelNotFound(args.model.clone()));
    }

    // Confirm removal if not using --yes flag
    if !args.yes {
        use std::io::{self, Write};
        print!(
            "Are you sure you want to remove model '{}'? (y/N): ",
            args.model
        );
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Removal cancelled.");
            return Ok(());
        }
    }

    println!("Removing model: {}", args.model);

    // Remove the model
    manager.remove(&args.model).await?;

    println!("Successfully removed model: {}", args.model);

    Ok(())
}

pub async fn handle_update(args: UpdateArgs, config: &FuseConfig) -> Result<()> {
    use crate::model::ModelManager;
    use crate::storage::Database;
    use crate::storage::ModelRepository;
    use std::sync::Arc;

    // Initialize database and repository
    let db_path = config.models_dir.join("fuse.redb");
    let db = Arc::new(Database::new(db_path)?);
    let repository = Arc::new(ModelRepository::new(db));

    // Create model manager
    let manager = ModelManager::new(repository, config.models_dir.clone());

    println!("Updating model: {}", args.model);

    // Update the model
    let metadata = manager.update(&args.model).await?;

    println!("Successfully updated model: {}", metadata.name);
    println!("  Version: {}", metadata.version);
    println!("  Size: {}", metadata.size_human_readable());
    if let Some(updated) = metadata.updated_at {
        println!("  Updated: {}", updated.format("%Y-%m-%d %H:%M:%S"));
    }

    Ok(())
}

pub async fn handle_list(args: ListArgs, config: &FuseConfig) -> Result<()> {
    use crate::model::{ModelManager, Provider, SortBy};
    use crate::storage::Database;
    use crate::storage::ModelRepository;
    use std::sync::Arc;

    // Initialize database and repository
    let db_path = config.models_dir.join("fuse.redb");
    let db = Arc::new(Database::new(db_path)?);
    let repository = Arc::new(ModelRepository::new(db));

    // Create model manager
    let manager = ModelManager::new(repository, config.models_dir.clone());

    // Parse source filter
    let source_filter = if let Some(source_str) = args.source {
        Some(
            source_str
                .parse::<Provider>()
                .map_err(crate::error::FuseError::ValidationError)?,
        )
    } else {
        None
    };

    // List models with filtering
    let models = manager.list_filtered(source_filter, SortBy::Name).await?;

    if models.is_empty() {
        println!("No models found.");
        println!("Pull a model with: fuse pull <model_name>");
        return Ok(());
    }

    println!("Found {} model(s):\n", models.len());

    for model in models {
        if args.verbose {
            println!("Name:         {}", model.name);
            println!("ID:           {}", model.id);
            println!("Source:       {}", model.source);
            println!("Version:      {}", model.version);
            println!("Size:         {}", model.size_human_readable());
            if let Some(arch) = &model.architecture {
                println!("Architecture: {}", arch);
            }
            if let Some(params) = model.parameter_count_human_readable() {
                println!("Parameters:   {}", params);
            }
            if let Some(quant) = &model.quantization {
                println!("Quantization: {}", quant);
            }
            println!(
                "Downloaded:   {}",
                model.downloaded_at.format("%Y-%m-%d %H:%M:%S")
            );
            if let Some(updated) = model.updated_at {
                println!("Updated:      {}", updated.format("%Y-%m-%d %H:%M:%S"));
            }
            if !model.tags.is_empty() {
                println!("Tags:         {}", model.tags.join(", "));
            }
            println!();
        } else {
            let size = model.size_human_readable();
            let source = model.source.provider.to_string();
            println!("  {} - {} ({})", model.name, size, source);
        }
    }

    Ok(())
}

pub async fn handle_inspect(args: InspectArgs, config: &FuseConfig) -> Result<()> {
    validation::validate_model_name(&args.model)?;

    println!("Inspecting model: {}", args.model);
    if args.json {
        println!("Output format: JSON");
    }
    println!("Note: Model inspection functionality will be implemented in task 8");
    println!(
        "Configuration: models_dir = {}",
        config.models_dir.display()
    );
    Ok(())
}

pub async fn handle_quantize(args: QuantizeArgs, config: &FuseConfig) -> Result<()> {
    use crate::cli::progress::Spinner;
    use crate::quantization::{QuantizationConfig, QuantizationMethod, QuantizationService};
    use crate::storage::Database;
    use tracing::{info, warn};

    validation::validate_model_name(&args.model)?;
    validation::validate_quantization_method(&args.method)?;

    if let Some(ref format) = args.format {
        validation::validate_quantization_format(format)?;
    }

    info!("Starting quantization for model: {}", args.model);

    // Load model metadata
    let dir_manager = crate::config::DirectoryManager::new().map_err(|e| {
        crate::error::FuseError::ConfigError(format!("Failed to initialize directories: {}", e))
    })?;
    let db_path = dir_manager.global_dir().join("fuse.db");
    let db = std::sync::Arc::new(Database::new(db_path)?);
    let repo = crate::storage::ModelRepository::new(db);

    let model_metadata = repo
        .get(&args.model)?
        .ok_or_else(|| crate::error::FuseError::ModelNotFound(args.model.clone()))?;

    let model_path = config.models_dir.join(&args.model);
    if !model_path.exists() {
        return Err(crate::error::FuseError::ModelNotFound(args.model.clone()));
    }

    // Parse quantization method
    let method = if let Some(format) = &args.format {
        // If format is specified, use it (e.g., Q4_0, Q5_1)
        QuantizationMethod::parse_method(format)?
    } else {
        // Otherwise use the method (e.g., gguf, gptq)
        match args.method.to_lowercase().as_str() {
            "gguf" => QuantizationMethod::Q4_0, // Default GGUF format
            "gptq" => QuantizationMethod::GPTQ,
            "awq" => QuantizationMethod::AWQ,
            "ggml" => QuantizationMethod::GGML,
            _ => {
                return Err(crate::error::FuseError::ValidationError(format!(
                    "Unknown quantization method: {}",
                    args.method
                )))
            }
        }
    };

    // Create quantization config
    let quant_config = QuantizationConfig::new(method);

    // For GPTQ, we might need calibration data
    if method == QuantizationMethod::GPTQ {
        println!("Note: GPTQ quantization requires calibration data.");
        println!("Using default calibration settings.");
        // In production, this would prompt for calibration dataset
    }

    // Determine output path
    let output_name = args
        .output
        .unwrap_or_else(|| format!("{}-{}", args.model, method.as_str()));
    let output_path = config.models_dir.join(&output_name);

    println!("Quantizing model: {}", args.model);
    println!("Method: {} ({})", method.as_str(), method.description());
    println!("Output: {}", output_name);
    println!(
        "Expected size reduction: ~{:.0}%",
        (1.0 - method.expected_size_reduction()) * 100.0
    );
    println!();

    // Create quantization service
    let service = QuantizationService::new(&config.models_dir)?;

    // Show progress
    let spinner = Spinner::new("Quantizing model");
    let spinner_handle = spinner.start().await;

    // Perform quantization
    match service
        .quantize(&model_path, &quant_config, &output_path)
        .await
    {
        Ok(result) => {
            spinner_handle
                .finish_with_message("Quantization completed successfully")
                .await;

            println!();
            println!("Quantization Results:");
            println!("  Method: {}", result.method.as_str());
            println!("  Input Size: {} MB", result.input_size_mb);
            println!("  Output Size: {} MB", result.output_size_mb);
            println!("  Compression Ratio: {:.2}x", result.compression_ratio);
            println!("  Duration: {} seconds", result.duration_secs);

            // Validate quantized model
            println!();
            println!("Validating quantized model...");
            match service.validate_quantized_model(&output_path).await {
                Ok(true) => {
                    println!("✓ Validation passed");

                    // Store metadata for quantized model
                    let mut quantized_metadata = model_metadata.clone();
                    quantized_metadata.name = output_name.clone();
                    quantized_metadata.quantization = Some(method.as_str().to_string());

                    repo.save(&quantized_metadata)?;

                    println!();
                    println!("Quantized model saved as: {}", output_name);
                    println!("You can now use it with: fuse run {}", output_name);
                }
                Ok(false) => {
                    warn!("Validation failed - quantized model may be corrupted");
                    println!("⚠ Warning: Validation failed");
                }
                Err(e) => {
                    warn!("Validation error: {}", e);
                    println!("⚠ Warning: Could not validate model: {}", e);
                }
            }
        }
        Err(e) => {
            spinner_handle
                .finish_with_message("Quantization failed")
                .await;
            eprintln!("✗ Quantization failed: {}", e);
            return Err(e);
        }
    }

    Ok(())
}

pub async fn handle_layer(args: LayerArgs, config: &FuseConfig) -> Result<()> {
    match args {
        LayerArgs::Inspect { model, wide } => {
            validation::validate_model_name(&model)?;
            println!("Inspecting layers for model: {}", model);
            if wide {
                println!("Output format: wide (detailed)");
            } else {
                println!("Output format: default (name, size, type)");
            }
            println!("Note: Layer inspection functionality will be implemented in task 13");
        }
        LayerArgs::Remove { model, layer_id } => {
            validation::validate_model_name(&model)?;
            println!("Removing layer {} from model: {}", layer_id, model);
            println!("Note: Layer removal functionality will be implemented in task 13");
        }
        LayerArgs::Add {
            model,
            layer_type,
            config: layer_config,
        } => {
            validation::validate_model_name(&model)?;
            validation::validate_layer_type(&layer_type)?;
            validation::validate_file_exists(&layer_config)?;
            println!("Adding {} layer to model: {}", layer_type, model);
            println!("Layer config: {}", layer_config.display());
            println!("Note: Layer addition functionality will be implemented in task 13");
        }
    }
    println!(
        "Configuration: models_dir = {}",
        config.models_dir.display()
    );
    Ok(())
}

pub async fn handle_comp_check(args: CompCheckArgs, config: &FuseConfig) -> Result<()> {
    if args.models.len() < 2 {
        return Err(crate::error::FuseError::ValidationError(
            "At least 2 models are required for compatibility check".to_string(),
        ));
    }

    for model in &args.models {
        validation::validate_model_name(model)?;
    }

    println!(
        "Checking compatibility for models: {}",
        args.models.join(", ")
    );
    println!("Note: Compatibility checking functionality will be implemented in task 14");
    println!(
        "Configuration: models_dir = {}",
        config.models_dir.display()
    );
    Ok(())
}

pub async fn handle_merge(args: MergeArgs, config: &FuseConfig) -> Result<()> {
    if args.models.len() < 2 {
        return Err(crate::error::FuseError::ValidationError(
            "At least 2 models are required for merging".to_string(),
        ));
    }

    for model in &args.models {
        validation::validate_model_name(model)?;
    }

    validation::validate_model_name(&args.output)?;
    validation::validate_merge_strategy(&args.strategy)?;

    println!("Merging models: {}", args.models.join(", "));
    println!("Output model: {}", args.output);
    println!("Strategy: {}", args.strategy);
    if let Some(weights) = args.weights {
        println!("Weights: {}", weights);
    }
    println!("Note: Model merging functionality will be implemented in task 15");
    println!(
        "Configuration: models_dir = {}",
        config.models_dir.display()
    );
    Ok(())
}

pub async fn handle_scan(
    args: ScanArgs,
    config: &FuseConfig,
    feature_manager: &FeatureFlagManager,
) -> Result<()> {
    use crate::config::feature_flags::Feature;

    if !feature_manager.is_enabled(Feature::VulnerabilityScanning) {
        return Err(crate::error::FuseError::FeatureDisabled(
            "Vulnerability scanning is not enabled. Enable it with: fuse features enable vulnerability-scanning".to_string(),
        ));
    }

    if !args.remote {
        validation::validate_model_name(&args.model)?;
    } else {
        validation::validate_url(&args.model)?;
    }

    validation::validate_scan_format(&args.format)?;

    println!("Scanning model: {}", args.model);
    if args.remote {
        println!("Scanning remote model");
    }
    println!("Output format: {}", args.format);
    if let Some(output) = args.output {
        println!("Output file: {}", output.display());
    }
    println!("Note: Vulnerability scanning functionality will be implemented in task 16");
    println!(
        "Configuration: models_dir = {}",
        config.models_dir.display()
    );
    Ok(())
}

pub async fn handle_remote(args: RemoteArgs, config: &FuseConfig) -> Result<()> {
    use crate::model::{Auth, RemoteEndpoint, RemoteEndpointRepository};
    use crate::storage::Database;

    // Initialize database and repository
    let db_path = config.models_dir.join("fuse.redb");
    let db = Database::new(db_path)?;
    let repo = RemoteEndpointRepository::new(db);

    match args {
        RemoteArgs::Add { name, url, api_key } => {
            validation::validate_url(&url)?;

            // Create remote endpoint
            let mut endpoint = RemoteEndpoint::new(name.clone(), url.clone());

            // Add authentication if provided
            if let Some(key) = api_key {
                endpoint = endpoint.with_auth(Auth::ApiKey(key));
            }

            // Add to repository
            repo.add(endpoint)?;

            println!("✓ Successfully added remote endpoint: {} -> {}", name, url);
        }
        RemoteArgs::Remove { name } => {
            // Remove from repository
            repo.remove(&name)?;

            println!("✓ Successfully removed remote endpoint: {}", name);
        }
        RemoteArgs::List => {
            let endpoints = repo.list()?;

            if endpoints.is_empty() {
                println!("No remote endpoints configured.");
                println!("\nAdd a remote endpoint with: fuse remote add <name> <url>");
                return Ok(());
            }

            println!("Remote endpoints:\n");
            for endpoint in endpoints {
                let status = if endpoint.enabled {
                    "enabled"
                } else {
                    "disabled"
                };
                let auth_status = if endpoint.auth.is_some() {
                    "authenticated"
                } else {
                    "no auth"
                };

                println!("  {} ({})", endpoint.name, status);
                println!("    URL: {}", endpoint.url);
                println!("    Auth: {}", auth_status);
                if let Some(desc) = &endpoint.description {
                    println!("    Description: {}", desc);
                }
                println!();
            }
        }
    }

    Ok(())
}

pub async fn handle_workflow(
    args: WorkflowArgs,
    config: &FuseConfig,
    feature_manager: &FeatureFlagManager,
) -> Result<()> {
    use crate::config::feature_flags::Feature;

    if !feature_manager.is_enabled(Feature::AgenticCoding) {
        return Err(crate::error::FuseError::FeatureDisabled(
            "Agentic coding is not enabled. Enable it with: fuse features enable agentic-coding"
                .to_string(),
        ));
    }

    match args {
        WorkflowArgs::Run { workflow, verbose } => {
            validation::validate_file_exists(&workflow)?;
            println!("Running workflow: {}", workflow.display());
            if verbose {
                println!("Verbose mode enabled");
            }
            println!("Note: Workflow execution will be implemented in task 11");
        }
        WorkflowArgs::List => {
            println!("Listing available workflows");
            println!("Note: Workflow listing will be implemented in task 11");
        }
        WorkflowArgs::Validate { workflow } => {
            validation::validate_file_exists(&workflow)?;
            println!("Validating workflow: {}", workflow.display());
            println!("Note: Workflow validation will be implemented in task 11");
        }
    }
    println!(
        "Configuration: models_dir = {}",
        config.models_dir.display()
    );
    Ok(())
}

pub async fn handle_ui(args: UiArgs, config: &FuseConfig) -> Result<()> {
    let port = args.port.unwrap_or(config.server.port);
    let host = args.host.unwrap_or_else(|| config.server.host.clone());

    validation::validate_port(port)?;

    println!("Starting web UI on {}:{}", host, port);
    if args.open {
        println!("Opening browser automatically");
    }
    println!("Note: Web UI will be implemented in task 9");
    Ok(())
}

pub async fn handle_history(args: HistoryArgs, config: &FuseConfig) -> Result<()> {
    if args.clear {
        use std::io::{self, Write};
        print!("Are you sure you want to clear all chat history? (y/N): ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Clear cancelled.");
            return Ok(());
        }

        println!("Clearing chat history");
        println!("Note: History clearing will be implemented in task 21");
    } else {
        println!("Showing chat history");
        if let Some(limit) = args.limit {
            println!("Limit: {} messages", limit);
        }
        if let Some(model) = args.model {
            validation::validate_model_name(&model)?;
            println!("Filter by model: {}", model);
        }
        println!("Note: History viewing will be implemented in task 21");
    }
    println!(
        "Configuration: models_dir = {}",
        config.models_dir.display()
    );
    Ok(())
}

pub async fn handle_mcp(
    args: McpArgs,
    config: &FuseConfig,
    feature_manager: &FeatureFlagManager,
) -> Result<()> {
    use crate::config::feature_flags::Feature;

    if !feature_manager.is_enabled(Feature::McpServer) {
        return Err(crate::error::FuseError::FeatureDisabled(
            "MCP server is not enabled. Enable it with: fuse features enable mcp-server"
                .to_string(),
        ));
    }

    match args {
        McpArgs::Start {
            port,
            config: mcp_config,
        } => {
            let port = port.unwrap_or(config.server.port + 1);
            validation::validate_port(port)?;

            println!("Starting MCP server on port {}", port);
            if let Some(config_path) = mcp_config {
                validation::validate_file_exists(&config_path)?;
                println!("Using config: {}", config_path.display());
            }
            println!("Note: MCP server will be implemented in task 17");
        }
        McpArgs::Stop => {
            println!("Stopping MCP server");
            println!("Note: MCP server control will be implemented in task 17");
        }
        McpArgs::Status => {
            println!("MCP server status");
            println!("Note: MCP server status will be implemented in task 17");
        }
    }
    Ok(())
}
