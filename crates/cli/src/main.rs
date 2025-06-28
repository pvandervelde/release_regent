//! Command-line interface for Release Regent
//!
//! This application provides local testing and configuration tools for Release Regent.

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
use tracing::{debug, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod errors;

use errors::{CliError, CliResult};

/// Release Regent CLI
#[derive(Parser, Debug)]
#[command(name = "rr")]
#[command(about = "Release Regent - Automated GitHub release management")]
#[command(long_about = r#"
Release Regent is a GitHub App that automates release management by creating
and updating release pull requests, calculating semantic versions, and
publishing GitHub releases.

This CLI provides tools for local testing and configuration management.
"#)]
#[command(version)]
struct Cli {
    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Configuration file path
    #[arg(short, long, global = true)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Generate sample configuration files
    Init(InitArgs),
    /// Process webhook events locally
    Run(RunArgs),
}

#[derive(Args, Debug)]
struct InitArgs {
    /// Output directory for generated files
    #[arg(short, long, default_value = ".")]
    output_dir: PathBuf,

    /// Configuration template type
    #[arg(short, long, default_value = "basic")]
    template: String,

    /// Overwrite existing files
    #[arg(long)]
    overwrite: bool,
}

#[derive(Args, Debug)]
struct RunArgs {
    /// Webhook event file (JSON format)
    #[arg(short, long)]
    event_file: PathBuf,

    /// Dry run mode (no actual operations)
    #[arg(short, long)]
    dry_run: bool,

    /// Configuration file path
    #[arg(short, long)]
    config_path: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> CliResult<()> {
    let cli = Cli::parse();

    // Initialize logging
    setup_logging(cli.verbose)?;

    info!("Starting Release Regent CLI");
    debug!("Parsed CLI arguments: {:?}", cli);

    // Execute the command
    match cli.command {
        Commands::Init(args) => execute_init(args).await,
        Commands::Run(args) => execute_run(args).await,
    }
}

/// Set up logging based on verbosity level
fn setup_logging(verbose: bool) -> CliResult<()> {
    let filter = if verbose { "debug" } else { "info" };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_file(false)
                .with_line_number(false),
        )
        .with(tracing_subscriber::EnvFilter::new(filter))
        .init();

    Ok(())
}

/// Execute the init command
async fn execute_init(args: InitArgs) -> CliResult<()> {
    info!("Initializing Release Regent configuration");
    debug!("Init args: {:?}", args);

    // Create output directory if it doesn't exist
    if !args.output_dir.exists() {
        tokio::fs::create_dir_all(&args.output_dir).await?;
        info!("Created output directory: {}", args.output_dir.display());
    }

    // Generate sample configuration
    let config_path = args.output_dir.join(".release-regent.yml");

    if config_path.exists() && !args.overwrite {
        return Err(CliError::config_file(
            "Configuration file already exists. Use --overwrite to replace it.",
        ));
    }

    let default_config = release_regent_core::config::ReleaseRegentConfig::default();
    let config_yaml = serde_yaml::to_string(&default_config)?;

    tokio::fs::write(&config_path, config_yaml).await?;
    info!("Generated configuration file: {}", config_path.display());

    // Generate sample webhook payload
    let webhook_path = args.output_dir.join("sample-webhook.json");
    let sample_webhook = generate_sample_webhook();

    tokio::fs::write(&webhook_path, sample_webhook).await?;
    info!("Generated sample webhook file: {}", webhook_path.display());

    println!("✅ Release Regent configuration initialized successfully!");
    println!("📝 Configuration file: {}", config_path.display());
    println!("🔗 Sample webhook: {}", webhook_path.display());
    println!();
    println!("Next steps:");
    println!(
        "1. Edit {} to customize your configuration",
        config_path.display()
    );
    println!(
        "2. Test with: rr run --event-file {}",
        webhook_path.display()
    );

    Ok(())
}

/// Execute the run command
async fn execute_run(args: RunArgs) -> CliResult<()> {
    info!("Processing webhook event locally");
    debug!("Run args: {:?}", args);

    if args.dry_run {
        info!("Running in dry-run mode - no actual operations will be performed");
    }

    // Load configuration
    let config_path = args
        .config_path
        .or_else(|| Some(PathBuf::from(".release-regent.yml")))
        .unwrap();

    if !config_path.exists() {
        return Err(CliError::config_file(format!(
            "Configuration file not found: {}. Run 'rr init' to create one.",
            config_path.display()
        )));
    }

    let _config =
        release_regent_core::config::ReleaseRegentConfig::load_from_file(&config_path).await?;
    info!("Loaded configuration from: {}", config_path.display());

    // Load webhook event
    if !args.event_file.exists() {
        return Err(CliError::invalid_argument(
            "--event-file",
            format!("File not found: {}", args.event_file.display()),
        ));
    }

    let _event_json = tokio::fs::read_to_string(&args.event_file).await?;
    info!("Loaded webhook event from: {}", args.event_file.display());

    // TODO: Parse webhook JSON and create WebhookEvent
    // TODO: Create ReleaseRegent instance and process webhook
    // This will be implemented in subsequent issues

    if args.dry_run {
        println!("🔍 Dry run completed - no changes made");
    } else {
        println!("✅ Webhook processing completed successfully");
    }

    Ok(())
}

/// Generate a sample webhook payload for testing
fn generate_sample_webhook() -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "action": "closed",
        "number": 42,
        "pull_request": {
            "id": 123456789,
            "number": 42,
            "state": "closed",
            "title": "feat: add new feature",
            "body": "This PR adds a new feature to the application.\n\n## Changes\n- Added feature X\n- Updated documentation",
            "merged": true,
            "merge_commit_sha": "abc123def456789",
            "base": {
                "ref": "main",
                "sha": "def456789abc123"
            },
            "head": {
                "ref": "feature/new-feature",
                "sha": "789abc123def456"
            }
        },
        "repository": {
            "id": 987654321,
            "name": "test-repo",
            "full_name": "owner/test-repo",
            "owner": {
                "login": "owner",
                "type": "User"
            },
            "default_branch": "main"
        }
    })).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        // This will be expanded with actual CLI parsing tests
        // when the command structure is finalized
    }

    #[test]
    fn test_sample_webhook_generation() {
        let webhook = generate_sample_webhook();
        assert!(!webhook.is_empty());

        // Verify it's valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&webhook).unwrap();
        assert!(parsed.get("action").is_some());
        assert!(parsed.get("pull_request").is_some());
        assert!(parsed.get("repository").is_some());
    }
}
