//! File-based configuration provider implementation.

use crate::errors::{ConfigProviderError, ConfigProviderResult};
use crate::formats::{ConfigFormat, FormatDetector};
use crate::validation::ConfigValidator;
use async_trait::async_trait;
use release_regent_core::{
    config::{ReleaseRegentConfig, VersioningStrategy},
    errors::CoreError,
    traits::{
        configuration_provider::{
            ConfigurationSource, LoadOptions, RepositoryConfig, ValidationResult,
        },
        ConfigurationProvider,
    },
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info, warn};

/// File-based configuration provider with YAML and TOML support
pub struct FileConfigurationProvider {
    /// Base directory for configuration files
    base_directory: PathBuf,
    /// Additional directories to search for configuration files
    search_directories: Vec<PathBuf>,
    /// Configuration validator
    validator: ConfigValidator,
    /// Configuration overrides
    overrides: HashMap<String, String>,
    /// Default format for files without clear extensions
    default_format: Option<ConfigFormat>,
    /// Specific global configuration file path
    global_config_path: Option<PathBuf>,
    /// Specific repository configuration file path
    repository_config_path: Option<PathBuf>,
    /// Whether to create missing configuration files
    create_missing: bool,
    /// Cached configurations
    config_cache: tokio::sync::RwLock<HashMap<String, CachedConfig>>,
}

/// Cached configuration entry
#[derive(Clone)]
struct CachedConfig {
    config: ReleaseRegentConfig,
    last_modified: std::time::SystemTime,
    file_path: PathBuf,
}

impl FileConfigurationProvider {
    /// Create a new file configuration provider
    pub async fn new<P: AsRef<Path>>(base_directory: P) -> ConfigProviderResult<Self> {
        let base_dir = base_directory.as_ref().to_path_buf();

        // Ensure base directory exists
        if !base_dir.exists() {
            fs::create_dir_all(&base_dir)
                .await
                .map_err(|e| ConfigProviderError::io_error("Failed to create base directory", e))?;
        }

        info!(
            "Created FileConfigurationProvider with base directory: {:?}",
            base_dir
        );

        Ok(Self {
            base_directory: base_dir,
            search_directories: Vec::new(),
            validator: ConfigValidator::new(),
            overrides: HashMap::new(),
            default_format: None,
            global_config_path: None,
            repository_config_path: None,
            create_missing: false,
            config_cache: tokio::sync::RwLock::new(HashMap::new()),
        })
    }

    /// Add a search directory for configuration files
    pub fn add_search_directory<P: AsRef<Path>>(&mut self, directory: P) {
        self.search_directories
            .push(directory.as_ref().to_path_buf());
    }

    /// Set the configuration validator
    pub fn set_validator(&mut self, validator: ConfigValidator) {
        self.validator = validator;
    }

    /// Set configuration overrides
    pub fn set_overrides(&mut self, overrides: HashMap<String, String>) {
        self.overrides = overrides;
    }

    /// Set the default format for files without clear extensions
    pub fn set_default_format(&mut self, format: ConfigFormat) {
        self.default_format = Some(format);
    }

    /// Set specific global configuration file path
    pub fn set_global_config_path<P: AsRef<Path>>(&mut self, path: P) {
        self.global_config_path = Some(path.as_ref().to_path_buf());
    }

    /// Set specific repository configuration file path
    pub fn set_repository_config_path<P: AsRef<Path>>(&mut self, path: P) {
        self.repository_config_path = Some(path.as_ref().to_path_buf());
    }

    /// Enable creation of missing configuration files
    pub fn enable_create_missing(&mut self) {
        self.create_missing = true;
    }

    /// Find configuration file in search directories
    async fn find_config_file(&self, filename: &str) -> ConfigProviderResult<Option<PathBuf>> {
        // Check specific paths first
        if filename == "global" {
            if let Some(path) = &self.global_config_path {
                if path.exists() {
                    return Ok(Some(path.clone()));
                }
            }
        }

        // Build list of directories to search
        let mut search_dirs = vec![self.base_directory.clone()];
        search_dirs.extend(self.search_directories.clone());

        // Common configuration file variations
        let variations = if filename == "global" {
            vec![
                "release-regent.yaml".to_string(),
                "release-regent.yml".to_string(),
                "release_regent.yaml".to_string(),
                "release_regent.yml".to_string(),
                "release-regent.toml".to_string(),
                "release_regent.toml".to_string(),
                "config.yaml".to_string(),
                "config.yml".to_string(),
                "config.toml".to_string(),
            ]
        } else {
            vec![
                format!("{}.yaml", filename),
                format!("{}.yml", filename),
                format!("{}.toml", filename),
            ]
        };

        for dir in search_dirs {
            for variation in &variations {
                let path = dir.join(variation);
                if path.exists() {
                    debug!("Found configuration file: {:?}", path);
                    return Ok(Some(path));
                }
            }
        }

        Ok(None)
    }

    /// Load configuration from file
    async fn load_config_from_file(
        &self,
        path: &Path,
    ) -> ConfigProviderResult<ReleaseRegentConfig> {
        debug!("Loading configuration from file: {:?}", path);

        // Check if file exists
        if !path.exists() {
            if self.create_missing {
                warn!("Configuration file not found, creating default: {:?}", path);
                return self.create_default_config_file(path).await;
            } else {
                return Err(ConfigProviderError::ConfigFileNotFound {
                    path: path.to_path_buf(),
                });
            }
        }

        // Read file content
        let content = fs::read_to_string(path)
            .await
            .map_err(|e| ConfigProviderError::io_error("Failed to read configuration file", e))?;

        // Detect format
        let format = FormatDetector::detect_from_path(path)
            .or_else(|_| {
                if let Some(default_format) = self.default_format {
                    Ok::<ConfigFormat, ConfigProviderError>(default_format)
                } else {
                    FormatDetector::detect_from_content(&content)
                }
            })
            .map_err(|e| ConfigProviderError::InvalidFormat {
                path: path.to_path_buf(),
                reason: format!("Could not detect format: {}", e),
            })?;

        // Parse content
        let mut config = format.parse(&content).map_err(|mut e| {
            // Update error with correct path if it's empty
            match &mut e {
                ConfigProviderError::ParseError {
                    path: error_path, ..
                } if error_path.as_os_str().is_empty() => {
                    *error_path = path.to_path_buf();
                }
                _ => {}
            }
            e
        })?;

        // Apply overrides
        self.apply_overrides(&mut config)?;

        info!("Successfully loaded configuration from: {:?}", path);
        Ok(config)
    }

    /// Create a default configuration file
    async fn create_default_config_file(
        &self,
        path: &Path,
    ) -> ConfigProviderResult<ReleaseRegentConfig> {
        let default_config = self.get_default_config().await?;

        // Detect format from path or use default
        let format = FormatDetector::detect_from_path(path)
            .or_else(|_| Ok::<ConfigFormat, ConfigProviderError>(self.default_format.unwrap_or(ConfigFormat::Yaml)))?;

        // Serialize configuration
        let content = format.serialize(&default_config)?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                ConfigProviderError::io_error("Failed to create config directory", e)
            })?;
        }

        // Write file
        fs::write(path, content)
            .await
            .map_err(|e| ConfigProviderError::io_error("Failed to write configuration file", e))?;

        info!("Created default configuration file: {:?}", path);
        Ok(default_config)
    }

    /// Apply configuration overrides
    fn apply_overrides(&self, config: &mut ReleaseRegentConfig) -> ConfigProviderResult<()> {
        for (key, value) in &self.overrides {
            match key.as_str() {
                "versioning.strategy" => {
                    // Convert string to VersioningStrategy enum
                    config.versioning.strategy = match value.as_str() {
                        "conventional" => VersioningStrategy::Conventional,
                        "external" => VersioningStrategy::External,
                        _ => config.versioning.strategy.clone(), // Keep existing if invalid
                    };
                }
                "branches.main_branch" => {
                    config.core.branches.main = value.clone();
                }
                "webhook.url" => {
                    // WebhookConfig only has 'url' and 'headers' fields
                    if config.notifications.webhook.is_none() {
                        config.notifications.webhook = Some(release_regent_core::config::WebhookConfig {
                            url: value.clone(),
                            headers: std::collections::HashMap::new(),
                        });
                    } else if let Some(webhook) = &mut config.notifications.webhook {
                        webhook.url = value.clone();
                    }
                }
                // Add more override patterns as needed
                _ => {
                    warn!("Unknown configuration override key: {}", key);
                }
            }
        }
        Ok(())
    }

    /// Get cached configuration if still valid
    async fn get_cached_config(
        &self,
        cache_key: &str,
        file_path: &Path,
    ) -> Option<ReleaseRegentConfig> {
        let cache = self.config_cache.read().await;

        if let Some(cached) = cache.get(cache_key) {
            // Check if file has been modified since cache
            if let Ok(metadata) = fs::metadata(file_path).await {
                if let Ok(modified) = metadata.modified() {
                    if modified <= cached.last_modified {
                        debug!("Using cached configuration for: {}", cache_key);
                        return Some(cached.config.clone());
                    }
                }
            }
        }

        None
    }

    /// Cache configuration
    async fn cache_config(
        &self,
        cache_key: String,
        config: ReleaseRegentConfig,
        file_path: PathBuf,
    ) {
        if let Ok(metadata) = fs::metadata(&file_path).await {
            if let Ok(modified) = metadata.modified() {
                let cached = CachedConfig {
                    config,
                    last_modified: modified,
                    file_path,
                };

                let mut cache = self.config_cache.write().await;
                cache.insert(cache_key, cached);
            }
        }
    }

    /// Clear cache for specific configuration
    async fn clear_cache(&self, cache_key: &str) {
        let mut cache = self.config_cache.write().await;
        cache.remove(cache_key);
    }

    /// Merge two configurations (repository overrides global)
    fn merge_configurations(
        &self,
        global: ReleaseRegentConfig,
        repository: ReleaseRegentConfig,
    ) -> ConfigProviderResult<ReleaseRegentConfig> {
        // For now, implement a simple merge where repository config overrides global config
        // In a real implementation, this would be more sophisticated

        let mut merged = global;

        // Override with repository-specific settings
        // Since all fields are required, we just replace the entire structures
        merged.core = repository.core;
        merged.versioning = repository.versioning;
        merged.releases = repository.releases;
        merged.notifications = repository.notifications;
        merged.error_handling = repository.error_handling;
        merged.release_pr = repository.release_pr;

        Ok(merged)
    }
}

#[async_trait]
impl ConfigurationProvider for FileConfigurationProvider {
    async fn load_global_config(
        &self,
        _options: LoadOptions,
    ) -> Result<ReleaseRegentConfig, CoreError> {
        let cache_key = "global".to_string();

        // Try to find global configuration file
        let config_path = match self
            .find_config_file("global")
            .await
            .map_err(|e| CoreError::config(e.to_string()))?
        {
            Some(path) => path,
            None => {
                if self.create_missing {
                    self.base_directory.join("release-regent.yaml")
                } else {
                    return Err(CoreError::config("Global configuration file not found"));
                }
            }
        };

        // Check cache first
        if let Some(cached_config) = self.get_cached_config(&cache_key, &config_path).await {
            return Ok(cached_config);
        }

        // Load configuration
        let config = self
            .load_config_from_file(&config_path)
            .await
            .map_err(|e| CoreError::config(e.to_string()))?;

        // Validate configuration
        let validation_result = self
            .validator
            .validate(&config)
            .map_err(|e| CoreError::config(e.to_string()))?;

        if !validation_result.is_valid {
            return Err(CoreError::config(format!(
                "Global configuration validation failed: {:?}",
                validation_result.errors
            )));
        }

        // Cache the configuration
        self.cache_config(cache_key, config.clone(), config_path)
            .await;

        Ok(config)
    }

    async fn load_repository_config(
        &self,
        owner: &str,
        repo: &str,
        _options: LoadOptions,
    ) -> Result<Option<RepositoryConfig>, CoreError> {
        let cache_key = format!("{}_{}", owner, repo);
        let filename = format!("{}-{}", owner, repo);

        // Try to find repository-specific configuration file
        let config_path = match self
            .find_config_file(&filename)
            .await
            .map_err(|e| CoreError::config(e.to_string()))?
        {
            Some(path) => path,
            None => {
                // Try generic repository config
                if let Some(repo_path) = &self.repository_config_path {
                    repo_path.clone()
                } else if self.create_missing {
                    self.base_directory.join(format!("{}.yaml", filename))
                } else {
                    // No repository config found - return None instead of error
                    return Ok(None);
                }
            }
        };

        // Check cache first
        if let Some(cached_config) = self.get_cached_config(&cache_key, &config_path).await {
            return Ok(Some(RepositoryConfig {
                config: cached_config,
                name: repo.to_string(),
                owner: owner.to_string(),
            }));
        }

        // Load configuration
        let config = self
            .load_config_from_file(&config_path)
            .await
            .map_err(|e| CoreError::config(e.to_string()))?;

        // Validate configuration
        let validation_result = self
            .validator
            .validate(&config)
            .map_err(|e| CoreError::config(e.to_string()))?;

        if !validation_result.is_valid {
            return Err(CoreError::config(format!(
                "Repository configuration validation failed: {:?}",
                validation_result.errors
            )));
        }

        // Cache the configuration
        self.cache_config(cache_key, config.clone(), config_path)
            .await;

        Ok(Some(RepositoryConfig {
            config,
            name: repo.to_string(),
            owner: owner.to_string(),
        }))
    }

    async fn get_merged_config(
        &self,
        owner: &str,
        repo: &str,
        options: LoadOptions,
    ) -> Result<ReleaseRegentConfig, CoreError> {
        // Load global configuration
        let global_config = self.load_global_config(options.clone()).await?;

        // Try to load repository-specific configuration
        match self.load_repository_config(owner, repo, options).await? {
            Some(repo_config) => {
                // Merge configurations
                self.merge_configurations(global_config, repo_config.config)
                    .map_err(|e| CoreError::config(e.to_string()))
            }
            None => {
                // If repository config doesn't exist, use global config
                debug!(
                    "Using global configuration for {}/{} (no repository-specific config)",
                    owner, repo
                );
                Ok(global_config)
            }
        }
    }

    async fn validate_config(
        &self,
        config: &ReleaseRegentConfig,
    ) -> Result<ValidationResult, CoreError> {
        let local_result = self
            .validator
            .validate(config)
            .map_err(|e| CoreError::config(e.to_string()))?;

        // Convert local ValidationResult to trait ValidationResult
        Ok(ValidationResult {
            is_valid: local_result.is_valid,
            errors: local_result.errors,
            warnings: local_result.warnings,
        })
    }

    async fn save_config(
        &self,
        config: &ReleaseRegentConfig,
        owner: Option<&str>,
        repo: Option<&str>,
        global: bool,
    ) -> Result<(), CoreError> {
        // Determine the file path and format
        let (file_path, config_format) = if global {
            let path = self
                .global_config_path
                .clone()
                .unwrap_or_else(|| self.base_directory.join("release-regent.yaml"));
            (path, "yaml")
        } else {
            match (owner, repo) {
                (Some(o), Some(r)) => {
                    let filename = format!("{}-{}", o, r);
                    let path = self
                        .repository_config_path
                        .clone()
                        .unwrap_or_else(|| self.base_directory.join(format!("{}.yaml", filename)));
                    (path, "yaml")
                }
                _ => {
                    return Err(CoreError::config(
                        "Owner and repo must be specified for repository configuration",
                    ));
                }
            }
        };

        // Parse format
        let format = match config_format.to_lowercase().as_str() {
            "yaml" | "yml" => ConfigFormat::Yaml,
            "toml" => ConfigFormat::Toml,
            _ => {
                return Err(CoreError::config(format!(
                    "Unsupported format: {}",
                    config_format
                )))
            }
        };

        // Serialize configuration
        let content = format
            .serialize(config)
            .map_err(|e| CoreError::config(e.to_string()))?;

        // Ensure parent directory exists
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| CoreError::config(format!("Failed to create directory: {}", e)))?;
        }

        // Write file
        fs::write(&file_path, content)
            .await
            .map_err(|e| CoreError::config(format!("Failed to write configuration: {}", e)))?;

        // Clear cache for this configuration
        let cache_key = if global {
            "global".to_string()
        } else {
            match (owner, repo) {
                (Some(o), Some(r)) => format!("{}_{}", o, r),
                _ => "global".to_string(),
            }
        };
        self.clear_cache(&cache_key).await;

        info!("Saved configuration to: {:?}", file_path);
        Ok(())
    }

    async fn list_repository_configs(
        &self,
        _options: LoadOptions,
    ) -> Result<Vec<RepositoryConfig>, CoreError> {
        let mut configs = Vec::new();

        // Search in all directories
        let mut search_dirs = vec![self.base_directory.clone()];
        search_dirs.extend(self.search_directories.clone());

        for dir in search_dirs {
            if let Ok(entries) = fs::read_dir(&dir).await {
                let mut entries = entries;
                while let Ok(Some(entry)) = entries.next_entry().await {
                    if let Some(file_name) = entry.file_name().to_str() {
                        // Look for files matching pattern: owner-repo.{yaml,yml,toml}
                        if let Some(stem) = file_name.split('.').next() {
                            if let Some((owner, repo)) = stem.split_once('-') {
                                if FormatDetector::is_supported_extension(
                                    file_name.split('.').last().unwrap_or(""),
                                ) {
                                    // Try to load the configuration to create a RepositoryConfig
                                    if let Ok(config) =
                                        self.load_config_from_file(&entry.path()).await
                                    {
                                        configs.push(RepositoryConfig {
                                            config,
                                            name: repo.to_string(),
                                            owner: owner.to_string(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Sort by owner/repo name
        configs.sort_by(|a, b| {
            (a.owner.clone(), a.name.clone()).cmp(&(b.owner.clone(), b.name.clone()))
        });
        configs.dedup_by(|a, b| a.owner == b.owner && a.name == b.name);
        Ok(configs)
    }

    async fn get_config_source(
        &self,
        owner: Option<&str>,
        repo: Option<&str>,
    ) -> Result<ConfigurationSource, CoreError> {
        let filename = match (owner, repo) {
            (Some(o), Some(r)) => format!("{}-{}", o, r),
            _ => "global".to_string(),
        };

        match self
            .find_config_file(&filename)
            .await
            .map_err(|e| CoreError::config(e.to_string()))?
        {
            Some(path) => {
                let format = FormatDetector::detect_from_path(&path)
                    .unwrap_or(ConfigFormat::Yaml)
                    .name()
                    .to_string();

                Ok(ConfigurationSource {
                    location: path.to_string_lossy().to_string(),
                    source_type: "file".to_string(),
                    format,
                    loaded_at: chrono::Utc::now(),
                })
            }
            None => Err(CoreError::config("Configuration file not found")),
        }
    }

    async fn reload_config(
        &self,
        owner: Option<&str>,
        repo: Option<&str>,
    ) -> Result<(), CoreError> {
        let cache_key = match (owner, repo) {
            (Some(o), Some(r)) => format!("{}_{}", o, r),
            _ => "global".to_string(),
        };

        self.clear_cache(&cache_key).await;
        info!("Cleared cache for configuration: {}", cache_key);
        Ok(())
    }

    async fn config_exists(
        &self,
        owner: Option<&str>,
        repo: Option<&str>,
    ) -> Result<bool, CoreError> {
        let filename = match (owner, repo) {
            (Some(o), Some(r)) => format!("{}-{}", o, r),
            _ => "global".to_string(),
        };

        match self.find_config_file(&filename).await {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => Err(CoreError::config(e.to_string())),
        }
    }

    fn supported_formats(&self) -> Vec<String> {
        FormatDetector::supported_formats()
            .iter()
            .map(|f| f.name().to_string())
            .collect()
    }

    async fn get_default_config(&self) -> Result<ReleaseRegentConfig, CoreError> {
        // Return a sensible default configuration
        Ok(ReleaseRegentConfig::default())
    }
}
