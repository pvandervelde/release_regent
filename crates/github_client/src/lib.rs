//! Crate for interacting with the GitHub REST API.
//!
//! This crate provides a client for making authenticated requests to GitHub,
//! authenticating as a GitHub App using its ID and private key.

use async_trait::async_trait;
use jsonwebtoken::EncodingKey;
use octocrab::{Octocrab, Result as OctocrabResult};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, instrument};

pub mod errors;
pub use errors::Error;

pub mod models;

pub mod pr_management;
pub mod release;

// Reference the tests module in the separate file
#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;

/// A client for interacting with the GitHub API, authenticated as a GitHub App.
///
/// This struct provides a high-level interface for GitHub API operations using
/// GitHub App authentication. It wraps an Octocrab client and provides methods
/// for repository management, installation token retrieval, and organization queries.
///
/// # Examples
///
/// ```rust,no_run
/// use github_client::{GitHubClient, create_app_client};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let app_id = 123456;
///     let private_key = "-----BEGIN RSA PRIVATE KEY-----\n...\n-----END RSA PRIVATE KEY-----";
///
///     let octocrab_client = create_app_client(app_id, private_key).await?;
///     let github_client = GitHubClient::new(octocrab_client);
///
///     let installations = github_client.list_installations().await?;
///     println!("Found {} installations", installations.len());
///
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct GitHubClient {
    /// The underlying Octocrab client used for API requests
    client: Octocrab,
}

impl GitHubClient {
    /// Fetches details for a specific repository.
    ///
    /// # Arguments
    ///
    /// * `owner` - The owner of the repository (user or organization name).
    /// * `repo` - The name of the repository.
    ///
    /// # Errors
    /// Returns an `Error::Octocrab` if the API call fails.
    #[instrument(skip(self), fields(owner = %owner, repo = %repo))]
    pub async fn get_repository(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<models::Repository, Error> {
        let result = self.client.repos(owner, repo).get().await;
        match result {
            Ok(r) => Ok(models::Repository::from(r)),
            Err(e) => {
                log_octocrab_error("Failed to get repository", e);
                return Err(Error::InvalidResponse);
            }
        }
    }

    /// Lists all installations for the authenticated GitHub App.
    ///
    /// This method retrieves all installations where the GitHub App is installed,
    /// which can be used to find the installation ID for a specific organization.
    ///
    /// # Returns
    ///
    /// A `Result` containing a vector of installation objects, or an error if the
    /// operation fails.
    ///
    /// # Errors
    ///
    /// Returns an `Error::InvalidResponse` if the API call fails or the response
    /// cannot be parsed.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use github_client::{GitHubClient, create_app_client};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// #     let app_id = 123456;
    /// #     let private_key = "-----BEGIN RSA PRIVATE KEY-----\n...\n-----END RSA PRIVATE KEY-----";
    /// #     let client_octocrab = create_app_client(app_id, private_key).await?;
    /// #     let client = GitHubClient::new(client_octocrab);
    ///
    ///     let installations = client.list_installations().await?;
    ///     for installation in installations {
    ///         println!("Installation ID: {}, Account: {}", installation.id, installation.account.login);
    ///     }
    ///
    /// #     Ok(())
    /// # }
    /// ```
    #[instrument(skip(self))]
    pub async fn list_installations(&self) -> Result<Vec<models::Installation>, Error> {
        info!("Listing installations for GitHub App using JWT authentication");

        // Use direct REST API call instead of octocrab's high-level method
        let result: OctocrabResult<Vec<octocrab::models::Installation>> =
            self.client.get("/app/installations", None::<&()>).await;

        match result {
            Ok(installations) => {
                let converted_installations: Vec<models::Installation> = installations
                    .into_iter()
                    .map(models::Installation::from)
                    .collect();

                info!(
                    count = converted_installations.len(),
                    "Successfully retrieved installations for GitHub App"
                );

                Ok(converted_installations)
            }
            Err(e) => {
                error!(
                    "Failed to list installations - this likely means JWT authentication failed"
                );
                log_octocrab_error("Failed to list installations", e);
                Err(Error::InvalidResponse)
            }
        }
    }

    /// Creates a new `GitHubClient` instance with the provided Octocrab client.
    ///
    /// This constructor wraps an existing Octocrab client that should already be
    /// configured with appropriate authentication (typically GitHub App JWT).
    ///
    /// # Arguments
    ///
    /// * `client` - An authenticated Octocrab client instance
    ///
    /// # Returns
    ///
    /// Returns a new `GitHubClient` instance ready for API operations.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use github_client::{GitHubClient, create_app_client};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let app_id = 123456;
    ///     let private_key = "-----BEGIN RSA PRIVATE KEY-----\n...\n-----END RSA PRIVATE KEY-----";
    ///
    ///     let octocrab_client = create_app_client(app_id, private_key).await?;
    ///     let github_client = GitHubClient::new(octocrab_client);
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn new(client: Octocrab) -> Self {
        Self { client }
    }
}

/// Authenticates with GitHub using an installation access token for a specific app installation.
///
/// This function retrieves an access token for a GitHub App installation and creates a new
/// `Octocrab` client authenticated with that token. It is useful for performing API operations
/// on behalf of a GitHub App installation.
///
/// # Arguments
///
/// * `octocrab` - An existing `Octocrab` client instance.
/// * `installation_id` - The ID of the GitHub App installation.
/// * `repository_owner` - The owner of the repository associated with the installation.
/// * `source_repository` - The name of the repository associated with the installation.
///
/// # Returns
///
/// A `Result` containing a new `Octocrab` client authenticated with the installation access token,
/// or an `Error` if the operation fails.
///
/// # Errors
///
/// This function returns an `Error` in the following cases:
/// - If the app installation cannot be found.
/// - If the access token cannot be created.
/// - If the new `Octocrab` client cannot be built.
///
/// # Example
///
/// ```rust,no_run
/// use github_client::{authenticate_with_access_token, Error};
/// use octocrab::Octocrab;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Error> {
///     let octocrab = Octocrab::builder().build().unwrap();
///     let installation_id = 12345678; // Replace with your installation ID
///     let repository_owner = "example-owner";
///     let source_repository = "example-repo";
///
///     let authenticated_client = authenticate_with_access_token(
///         &octocrab,
///         installation_id,
///         repository_owner,
///         source_repository,
///     )
///     .await?;
///
///     // Use `authenticated_client` to perform API operations
///     Ok(())
/// }
/// ```
#[instrument]
pub async fn authenticate_with_access_token(
    octocrab: &Octocrab,
    installation_id: u64,
    repository_owner: &str,
    source_repository: &str,
) -> Result<Octocrab, Error> {
    debug!(
        repository_owner = repository_owner,
        repository = source_repository,
        installation_id,
        "Finding installation"
    );

    let (api_with_token, _) = octocrab
        .installation_and_token(installation_id.into())
        .await
        .map_err(|_| {
            error!(
                repository_owner = repository_owner,
                repository = source_repository,
                installation_id,
                "Failed to create a token for the installation",
            );

            Error::InvalidResponse
        })?;

    info!(
        repository_owner = repository_owner,
        repository = source_repository,
        installation_id,
        "Created access token for installation",
    );

    Ok(api_with_token)
}

/// Creates an `Octocrab` client authenticated as a GitHub App using a JWT token.
///
/// This function generates a JSON Web Token (JWT) for the specified GitHub App ID and private key,
/// and uses it to create an authenticated `Octocrab` client. The client can then be used to perform
/// API operations on behalf of the GitHub App.
///
/// # Arguments
///
/// * `app_id` - The ID of the GitHub App.
/// * `private_key` - The private key associated with the GitHub App, in PEM format.
///
/// # Returns
///
/// A `Result` containing an authenticated `Octocrab` client, or an `Error` if the operation fails.
///
/// # Errors
///
/// This function returns an `Error` in the following cases:
/// - If the private key cannot be parsed.
/// - If the JWT token cannot be created.
/// - If the `Octocrab` client cannot be built.
///
/// # Example
///
/// ```rust,no_run
/// use github_client::{create_app_client, Error};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Error> {
///     let app_id = 123456; // Replace with your GitHub App ID
///     let private_key = r#"
/// -----BEGIN RSA PRIVATE KEY-----
/// ...
/// -----END RSA PRIVATE KEY-----
/// "#; // Replace with your GitHub App private key
///
///     let client = create_app_client(app_id, private_key).await?;
///
///     // Use `client` to perform API operations
///     Ok(())
/// }
/// ```
#[instrument(skip(private_key))]
pub async fn create_app_client(app_id: u64, private_key: &str) -> Result<Octocrab, Error> {
    info!(
        app_id = app_id,
        key_length = private_key.len(),
        key_starts_with = &private_key[..27], // "-----BEGIN RSA PRIVATE KEY"
        "Creating GitHub App client with provided credentials"
    );

    let key = EncodingKey::from_rsa_pem(private_key.as_bytes()).map_err(|e| {
        error!(
            app_id = app_id,
            error = %e,
            "Failed to parse RSA private key - key format is invalid"
        );
        Error::AuthError(
            format!("Failed to translate the private key. Error was: {}", e).to_string(),
        )
    })?;

    info!(app_id = app_id, "Successfully parsed RSA private key");

    let octocrab = Octocrab::builder()
        .app(app_id.into(), key)
        .build()
        .map_err(|e| {
            error!(
                app_id = app_id,
                error = ?e,
                "Failed to build Octocrab client with GitHub App credentials"
            );
            Error::AuthError("Failed to get a personal token for the app install.".to_string())
        })?;

    info!(app_id = app_id, "Successfully created GitHub App client");

    Ok(octocrab)
}

/// Creates an Octocrab client authenticated with a personal access token.
///
/// This function creates a GitHub API client using a personal access token
/// for authentication. This is useful for operations that don't require
/// GitHub App authentication.
///
/// # Arguments
///
/// * `token` - A GitHub personal access token
///
/// # Returns
///
/// Returns a `Result` containing an authenticated `Octocrab` client, or an `Error`
/// if the client cannot be built.
///
/// # Errors
///
/// This function returns an `Error::ApiError` if the Octocrab client cannot be
/// constructed with the provided token.
///
/// # Examples
///
/// ```rust,no_run
/// use github_client::create_token_client;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let token = "ghp_xxxxxxxxxxxxxxxxxxxx"; // Your GitHub PAT
///     let client = create_token_client(token)?;
///
///     // Use client for API operations
///     Ok(())
/// }
/// ```
#[instrument(skip(token))]
pub fn create_token_client(token: &str) -> Result<Octocrab, Error> {
    Octocrab::builder()
        .personal_token(token.to_string())
        .build()
        .map_err(|_| Error::ApiError())
}

/// Helper function to log Octocrab errors with appropriate detail.
///
/// This function examines the type of Octocrab error and logs relevant
/// information for debugging purposes. It handles different error types
/// with appropriate context and formatting.
fn log_octocrab_error(message: &str, e: octocrab::Error) {
    match e {
        octocrab::Error::GitHub { source, backtrace } => {
            let err = source;
            error!(
                error_message = err.message,
                backtrace = backtrace.to_string(),
                "{}. Received an error from GitHub",
                message
            )
        }
        octocrab::Error::UriParse { source, backtrace } => error!(
            error_message = source.to_string(),
            backtrace = backtrace.to_string(),
            "{}. Failed to parse URI.",
            message
        ),

        octocrab::Error::Uri { source, backtrace } => error!(
            error_message = source.to_string(),
            backtrace = backtrace.to_string(),
            "{}, Failed to parse URI.",
            message
        ),
        octocrab::Error::InvalidHeaderValue { source, backtrace } => error!(
            error_message = source.to_string(),
            backtrace = backtrace.to_string(),
            "{}. One of the header values was invalid.",
            message
        ),
        octocrab::Error::InvalidUtf8 { source, backtrace } => error!(
            error_message = source.to_string(),
            backtrace = backtrace.to_string(),
            "{}. The message wasn't valid UTF-8.",
            message,
        ),
        _ => error!(error_message = e.to_string(), message),
    };
}
