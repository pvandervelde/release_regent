use thiserror::Error;

/// Errors that can occur in CLI operations
#[derive(Error, Debug)]
pub enum CliError {
    /// Core operation errors
    #[error("Core operation failed: {source}")]
    Core {
        #[from]
        source: release_regent_core::CoreError,
    },

    /// GitHub client errors
    #[error("GitHub operation failed: {source}")]
    GitHub {
        #[from]
        source: release_regent_github_client::GitHubError,
    },

    /// Configuration file errors
    #[error("Configuration file error: {message}")]
    ConfigFile { message: String },

    /// Command execution errors
    #[error("Command execution failed: {command} - {message}")]
    CommandExecution { command: String, message: String },

    /// File I/O errors
    #[error("File operation failed: {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },

    /// YAML parsing errors
    #[error("YAML parsing failed: {source}")]
    YamlParsing {
        #[from]
        source: serde_yaml::Error,
    },

    /// Invalid command arguments
    #[error("Invalid argument: {argument} - {message}")]
    InvalidArgument { argument: String, message: String },

    /// Missing required files or dependencies
    #[error("Missing dependency: {dependency} - {message}")]
    MissingDependency { dependency: String, message: String },
}

impl CliError {
    /// Create a new configuration file error
    pub fn config_file(message: impl Into<String>) -> Self {
        Self::ConfigFile {
            message: message.into(),
        }
    }

    /// Create a new command execution error
    pub fn command_execution(command: impl Into<String>, message: impl Into<String>) -> Self {
        Self::CommandExecution {
            command: command.into(),
            message: message.into(),
        }
    }

    /// Create a new invalid argument error
    pub fn invalid_argument(argument: impl Into<String>, message: impl Into<String>) -> Self {
        Self::InvalidArgument {
            argument: argument.into(),
            message: message.into(),
        }
    }

    /// Create a new missing dependency error
    pub fn missing_dependency(dependency: impl Into<String>, message: impl Into<String>) -> Self {
        Self::MissingDependency {
            dependency: dependency.into(),
            message: message.into(),
        }
    }
}

/// Result type for CLI operations
pub type CliResult<T> = Result<T, CliError>;

#[cfg(test)]
#[path = "errors_tests.rs"]
mod tests;
