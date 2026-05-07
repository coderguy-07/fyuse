pub mod config;
pub mod features;
pub mod init;
pub mod merge_models;
pub mod model;
pub mod queue;
pub mod rag;
pub mod serve;

use crate::cli::commands::*;
#[allow(unused_imports)]
use crate::cli::{Commands, LayerAction, McpAction, RemoteAction, WorkflowAction};
use crate::config::feature_flags::FeatureFlagManager;
use crate::config::FuseConfig;
use crate::error::Result;

/// Main command handler that routes to specific handlers
pub async fn handle_command(
    command: Commands,
    config: FuseConfig,
    feature_manager: FeatureFlagManager,
) -> Result<()> {
    match command {
        Commands::Init { from_file, yes } => {
            let args = InitArgs { from_file, yes };
            init::handle(args)?;
        }
        Commands::Pull {
            model,
            source,
            resume,
        } => {
            let args = PullArgs {
                model,
                source,
                resume,
            };
            model::handle_pull(args, &config, &feature_manager).await?;
        }
        Commands::Run { model, port } => {
            let args = RunArgs { model, port };
            model::handle_run(args, &config, &feature_manager).await?;
        }
        Commands::Rm { model, yes } => {
            let args = RmArgs { model, yes };
            model::handle_rm(args, &config).await?;
        }
        Commands::Update { model } => {
            let args = UpdateArgs { model };
            model::handle_update(args, &config).await?;
        }
        Commands::List { verbose, source } => {
            let args = ListArgs { verbose, source };
            model::handle_list(args, &config).await?;
        }
        Commands::Inspect { model, json } => {
            let args = InspectArgs { model, json };
            model::handle_inspect(args, &config).await?;
        }
        Commands::Quantize {
            model,
            method,
            format,
            output,
        } => {
            let args = QuantizeArgs {
                model,
                method,
                format,
                output,
            };
            model::handle_quantize(args, &config).await?;
        }
        Commands::Layer { action } => {
            let args = match action {
                LayerAction::Inspect { model, wide } => LayerArgs::Inspect { model, wide },
                LayerAction::Remove { model, layer_id } => LayerArgs::Remove { model, layer_id },
                LayerAction::Add {
                    model,
                    layer_type,
                    config: layer_config,
                } => LayerArgs::Add {
                    model,
                    layer_type,
                    config: layer_config,
                },
            };
            model::handle_layer(args, &config).await?;
        }
        Commands::CompCheck { models } => {
            let args = CompCheckArgs { models };
            model::handle_comp_check(args, &config).await?;
        }
        Commands::Merge {
            models,
            output,
            strategy,
            weights,
        } => {
            let args = MergeArgs {
                models,
                output,
                strategy,
                weights,
            };
            model::handle_merge(args, &config).await?;
        }
        Commands::Scan {
            model,
            remote,
            format,
            output,
        } => {
            let args = ScanArgs {
                model,
                remote,
                format,
                output,
            };
            model::handle_scan(args, &config, &feature_manager).await?;
        }
        Commands::Remote { action } => {
            let args = match action {
                RemoteAction::Add { name, url, api_key } => RemoteArgs::Add { name, url, api_key },
                RemoteAction::Remove { name } => RemoteArgs::Remove { name },
                RemoteAction::List => RemoteArgs::List,
            };
            model::handle_remote(args, &config).await?;
        }
        Commands::Workflow { action } => {
            let args = match action {
                WorkflowAction::Run { workflow, verbose } => {
                    WorkflowArgs::Run { workflow, verbose }
                }
                WorkflowAction::List => WorkflowArgs::List,
                WorkflowAction::Validate { workflow } => WorkflowArgs::Validate { workflow },
            };
            model::handle_workflow(args, &config, &feature_manager).await?;
        }
        Commands::Ui { port, host, open } => {
            let args = UiArgs { port, host, open };
            model::handle_ui(args, &config).await?;
        }
        Commands::History {
            limit,
            clear,
            model,
        } => {
            let args = HistoryArgs {
                limit,
                clear,
                model,
            };
            model::handle_history(args, &config).await?;
        }
        Commands::Mcp { action } => {
            let args = match action {
                McpAction::Start {
                    port,
                    config: mcp_config,
                } => McpArgs::Start {
                    port,
                    config: mcp_config,
                },
                McpAction::Stop => McpArgs::Stop,
                McpAction::Status => McpArgs::Status,
            };
            model::handle_mcp(args, &config, &feature_manager).await?;
        }
        Commands::Features { action } => {
            use crate::cli::FeatureAction;
            let args = match action {
                FeatureAction::List => FeatureArgs::List,
                FeatureAction::Enable { feature } => FeatureArgs::Enable { feature },
                FeatureAction::Disable { feature } => FeatureArgs::Disable { feature },
            };
            features::handle(args, feature_manager, config)?;
        }
        Commands::Learn {
            path,
            verbose,
            force,
        } => {
            rag::handle_learn(&path, verbose, force).await?;
        }
        Commands::Queue { action } => {
            let args = match action {
                crate::cli::QueueAction::Stats => QueueArgs::Stats,
                crate::cli::QueueAction::Flush => QueueArgs::Flush,
                crate::cli::QueueAction::Health => QueueArgs::Health,
            };
            queue::handle_queue(args, &config).await?;
        }
        Commands::System { action } => {
            let args = match action {
                crate::cli::SystemAction::Check => SystemArgs::Check,
                crate::cli::SystemAction::Resources => SystemArgs::Resources,
                crate::cli::SystemAction::Health => SystemArgs::Health,
            };
            queue::handle_system(args, &config).await?;
        }
        Commands::Monitor { action } => {
            let args = match action {
                crate::cli::MonitorAction::Performance => MonitorArgs::Performance,
                crate::cli::MonitorAction::Resources => MonitorArgs::Resources,
                crate::cli::MonitorAction::Queue => MonitorArgs::Queue,
            };
            queue::handle_monitor(args, &config).await?;
        }
        Commands::Backup { action } => {
            let args = match action {
                crate::cli::BackupAction::Create => BackupArgs::Create,
                crate::cli::BackupAction::Restore { file } => BackupArgs::Restore { file },
                crate::cli::BackupAction::List => BackupArgs::List,
            };
            queue::handle_backup(args, &config).await?;
        }
        Commands::Debug { action } => {
            let args = match action {
                crate::cli::DebugAction::Logs => DebugArgs::Logs,
                crate::cli::DebugAction::Config => DebugArgs::Config,
                crate::cli::DebugAction::Connections => DebugArgs::Connections,
                crate::cli::DebugAction::Models => DebugArgs::Models,
            };
            queue::handle_debug(args, &config).await?;
        }
        Commands::MergeModels {
            models,
            output,
            strategy,
            weights,
            slerp_t,
            base_model,
            validate,
            preserve_metadata,
        } => {
            let args = MergeModelsArgs {
                models,
                output,
                strategy,
                weights,
                slerp_t,
                base_model,
                validate,
                preserve_metadata,
            };
            merge_models::handle_merge_models(args, &config).await?;
        }
        Commands::Config { path, validate } => {
            let args = ConfigArgs { path, validate };
            config::handle(args, &config)?;
        }
        Commands::Serve {
            port,
            host,
            ollama_api,
            openai_api,
            anthropic_api,
        } => {
            let args = ServeArgs {
                port,
                host,
                ollama_api,
                openai_api,
                anthropic_api,
            };
            serve::handle_serve(args, &config).await?;
        }
        Commands::Doctor => {
            serve::handle_doctor(&config).await?;
        }
    }

    Ok(())
}
