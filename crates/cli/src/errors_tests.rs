use super::*;

#[test]
fn test_command_execution_error_creation() {
    let error = CliError::command_execution("init", "Failed to create directory");

    match error {
        CliError::CommandExecution {
            ref command,
            ref message,
        } => {
            assert_eq!(command, "init");
            assert_eq!(message, "Failed to create directory");
        }
        _ => panic!("Expected CommandExecution error"),
    }

    assert_eq!(
        error.to_string(),
        "Command execution failed: init - Failed to create directory"
    );
}

#[test]
fn test_config_file_error_creation() {
    let error = CliError::config_file("Configuration file not found");

    match error {
        CliError::ConfigFile { ref message } => {
            assert_eq!(message, "Configuration file not found");
        }
        _ => panic!("Expected ConfigFile error"),
    }

    assert_eq!(
        error.to_string(),
        "Configuration file error: Configuration file not found"
    );
}

#[test]
fn test_invalid_argument_error_creation() {
    let error = CliError::invalid_argument("--config", "File does not exist");

    match error {
        CliError::InvalidArgument {
            ref argument,
            ref message,
        } => {
            assert_eq!(argument, "--config");
            assert_eq!(message, "File does not exist");
        }
        _ => panic!("Expected InvalidArgument error"),
    }

    assert_eq!(
        error.to_string(),
        "Invalid argument: --config - File does not exist"
    );
}

#[test]
fn test_io_error_conversion() {
    let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
    let cli_error = CliError::from(io_error);

    match cli_error {
        CliError::Io { .. } => {
            // Expected
        }
        _ => panic!("Expected Io error from std::io::Error"),
    }
}

#[test]
fn test_missing_dependency_error_creation() {
    let error = CliError::missing_dependency("git", "Git command not found in PATH");

    match error {
        CliError::MissingDependency {
            ref dependency,
            ref message,
        } => {
            assert_eq!(dependency, "git");
            assert_eq!(message, "Git command not found in PATH");
        }
        _ => panic!("Expected MissingDependency error"),
    }

    assert_eq!(
        error.to_string(),
        "Missing dependency: git - Git command not found in PATH"
    );
}
