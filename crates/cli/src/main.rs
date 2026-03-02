//! Command-line interface for Release Regent
//!
//! This application provides local testing and configuration tools for Release Regent.

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
use tracing::{debug, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod errors;
mod factory;
mod version_calculator;

use errors::{CliError, CliResult};

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;

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
    /// Test parsing and changelog generation from Git history
    Test(TestArgs),
    /// Generate test data files for development workflows
    Generate(GenerateArgs),
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

    /// GitHub event type (e.g. pull_request, push).  Defaults to pull_request.
    #[arg(long, default_value = "pull_request")]
    event_type: String,

    /// Dry run mode (no actual operations)
    #[arg(short, long)]
    dry_run: bool,

    /// Use mock dependencies instead of real GitHub API credentials
    #[arg(long)]
    mock: bool,

    /// Configuration file path
    #[arg(short, long)]
    config_path: Option<PathBuf>,
}

#[derive(Args, Debug)]
struct TestArgs {
    /// Number of commits to analyse from current HEAD
    #[arg(short = 'n', long, default_value = "10")]
    commits: usize,

    /// Starting commit SHA (defaults to HEAD)
    #[arg(short, long)]
    from: Option<String>,

    /// Show detailed commit parsing
    #[arg(short, long)]
    verbose: bool,

    /// Current version to calculate next version from
    #[arg(long)]
    current_version: Option<String>,
}

#[derive(Args, Debug)]
struct GenerateArgs {
    /// Output directory for generated files
    #[arg(short, long, default_value = ".")]
    output_dir: PathBuf,

    /// Type of test data to generate: webhook, config, or all
    #[arg(short, long, default_value = "all")]
    kind: String,

    /// Overwrite existing files
    #[arg(long)]
    overwrite: bool,
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

/// Execute the run command, routing to mock or production processor via `--mock`.
async fn execute_run(args: RunArgs) -> CliResult<()> {
    info!("Processing webhook event locally");
    debug!("Run args: {:?}", args);

    if args.dry_run {
        info!("Running in dry-run mode — no actual operations will be performed");
    }

    // Validate that an explicit config file, when provided, actually exists.
    if let Some(ref config_path) = args.config_path {
        if !config_path.exists() {
            return Err(CliError::config_file(format!(
                "Configuration file not found: {}. Run 'rr init' to create one.",
                config_path.display()
            )));
        }
    }

    // Load and parse the webhook JSON file.
    if !args.event_file.exists() {
        return Err(CliError::invalid_argument(
            "--event-file",
            format!("File not found: {}", args.event_file.display()),
        ));
    }

    let event_json = tokio::fs::read_to_string(&args.event_file).await?;
    info!("Loaded webhook event from: {}", args.event_file.display());

    let payload: serde_json::Value = serde_json::from_str(&event_json)
        .map_err(|e| CliError::invalid_argument("--event-file", format!("Invalid JSON: {}", e)))?;

    // "action" lives at the top of every GitHub webhook payload.
    let action = payload["action"].as_str().unwrap_or("unknown").to_string();

    let event = release_regent_core::webhook::WebhookEvent::new(
        args.event_type.clone(),
        action,
        payload,
        std::collections::HashMap::new(), // no HTTP headers for local file input
    );

    info!(
        "Parsed webhook event: type={}, dry_run={}, mock={}",
        args.event_type, args.dry_run, args.mock
    );

    // Route to the appropriate processor based on the --mock flag.
    if args.mock {
        info!("Using mock processor for local development");
        let processor = factory::create_mock_processor();
        run_with_processor(processor, event, args.dry_run).await
    } else {
        info!("Using production processor with GitHub API credentials from environment");
        let processor = factory::create_production_processor().await?;
        run_with_processor(processor, event, args.dry_run).await
    }
}

/// Drive a [`ReleaseRegentProcessor`] through a single webhook event.
///
/// Generic over the four processor trait parameters so it compiles for both the
/// mock and production processor flavours without requiring a trait object.
async fn run_with_processor<G, C, V, W>(
    processor: release_regent_core::ReleaseRegentProcessor<G, C, V, W>,
    event: release_regent_core::webhook::WebhookEvent,
    dry_run: bool,
) -> CliResult<()>
where
    G: release_regent_core::traits::GitHubOperations,
    C: release_regent_core::traits::ConfigurationProvider,
    V: release_regent_core::traits::VersionCalculator,
    W: release_regent_core::traits::WebhookValidator,
{
    if dry_run {
        info!("Dry-run mode: skipping webhook processing");
        println!("🔍 Dry run completed - no changes made");
        return Ok(());
    }

    processor.process_webhook(event).await?;
    info!("Webhook processing completed successfully");
    println!("✅ Webhook processing completed successfully");
    Ok(())
}

/// Execute the generate command — write test data files to the output directory.
async fn execute_generate(args: GenerateArgs) -> CliResult<()> {
    info!("Generating test data files");
    debug!("Generate args: {:?}", args);

    tokio::fs::create_dir_all(&args.output_dir).await?;

    let wants_webhook = args.kind == "webhook" || args.kind == "all";
    let wants_config = args.kind == "config" || args.kind == "all";

    if wants_webhook {
        let path = args.output_dir.join("sample-webhook.json");
        if path.exists() && !args.overwrite {
            return Err(CliError::config_file(format!(
                "{} already exists. Use --overwrite to replace it.",
                path.display()
            )));
        }
        tokio::fs::write(&path, generate_sample_webhook()).await?;
        info!("Generated webhook fixture: {}", path.display());
        println!("📄 Webhook fixture: {}", path.display());
    }

    if wants_config {
        let path = args.output_dir.join("sample-config.yml");
        if path.exists() && !args.overwrite {
            return Err(CliError::config_file(format!(
                "{} already exists. Use --overwrite to replace it.",
                path.display()
            )));
        }
        let config = release_regent_core::config::ReleaseRegentConfig::default();
        let yaml = serde_yaml::to_string(&config)?;
        tokio::fs::write(&path, yaml).await?;
        info!("Generated config fixture: {}", path.display());
        println!("📄 Config fixture: {}", path.display());
    }

    println!("✅ Test data generated in {}", args.output_dir.display());
    Ok(())
}

/// Execute the test command
async fn execute_test(args: TestArgs) -> CliResult<()> {
    use release_regent_core::{changelog::ChangelogGenerator, versioning::VersionCalculator};

    info!("Testing conventional commit parsing and changelog generation");
    debug!("Test args: {:?}", args);

    // Get commit messages from git log
    let commits = get_recent_commits(args.commits, args.from.as_deref()).await?;

    if commits.is_empty() {
        println!("No commits found to analyze.");
        return Ok(());
    }

    println!("Analyzing {} commits...\n", commits.len());

    // Parse conventional commits
    let parsed_commits = VersionCalculator::parse_conventional_commits(&commits);

    if args.verbose {
        println!("=== Parsed Commits ===");
        for commit in &parsed_commits {
            println!(
                "• {} ({}): {}",
                commit.commit_type,
                commit.scope.as_deref().unwrap_or("no scope"),
                commit.description
            );
            if commit.breaking_change {
                println!("  ⚠️  BREAKING CHANGE");
            }
            println!("  SHA: {}", commit.sha);
            println!();
        }
    }

    // Calculate version bump
    let current_version = args.current_version.clone();
    let calculator = if let Some(current) = args.current_version {
        let version = VersionCalculator::parse_version(&current).map_err(|e| {
            CliError::invalid_argument("current_version", format!("Invalid current version: {}", e))
        })?;
        VersionCalculator::new(Some(version))
    } else {
        VersionCalculator::new(None)
    };

    match calculator.calculate_next_version(&parsed_commits) {
        Ok(next_version) => {
            println!("=== Version Calculation ===");
            if let Some(current) = current_version {
                println!("Current version: {}", current);
            } else {
                println!("Current version: (none - initial release)");
            }
            println!("Next version: {}", next_version);
            println!();
        }
        Err(e) => {
            println!("Version calculation failed: {}", e);
        }
    }

    // Generate changelog
    let generator = ChangelogGenerator::new();
    let changelog = generator.generate_changelog(&parsed_commits);

    println!("=== Generated Changelog ===");
    println!("{}", changelog);

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

/// Main entry point for the CLI application
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
        Commands::Test(args) => execute_test(args).await,
        Commands::Generate(args) => execute_generate(args).await,
    }
}

/// Get recent commits from git log
async fn get_recent_commits(count: usize, from: Option<&str>) -> CliResult<Vec<(String, String)>> {
    use std::process::Command;

    let mut cmd = Command::new("git");
    cmd.arg("log").arg("--oneline").arg(format!("-{}", count));

    if let Some(from_sha) = from {
        cmd.arg(format!("{}..HEAD", from_sha));
    }

    let output = cmd.output()
        .map_err(|e| CliError::command_execution(
            "git",
            format!("Failed to execute git command. Make sure git is installed and you're in a git repository. Error: {}", e),
        ))?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        return Err(CliError::command_execution(
            "git",
            format!("Git command failed: {}", error_msg),
        ));
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    let commits: Vec<(String, String)> = output_str
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(2, ' ').collect();
            if parts.len() == 2 {
                Some((parts[0].to_string(), parts[1].to_string()))
            } else {
                None
            }
        })
        .collect();

    Ok(commits)
}
