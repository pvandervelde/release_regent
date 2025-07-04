//! # Models
//!
//! This module contains the data models used throughout the Merge Warden core.
//!
//! These models represent the core entities that Merge Warden works with, such as
//! pull requests, comments, and labels. They are designed to be serializable and
//! deserializable to facilitate integration with Git provider APIs.

use serde::{Deserialize, Serialize};
use url::Url;

#[cfg(test)]
#[path = "models_tests.rs"]
mod tests;

/// Represents a GitHub account (user or organization).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Account {
    /// The unique ID of the account
    pub id: u64,
    /// The login name of the account
    pub login: String,
    /// The type of account (User or Organization)
    #[serde(rename = "type")]
    pub account_type: String,
    /// The node ID for GraphQL operations
    pub node_id: String,
}

// Remove the Account conversion since octocrab::models::Account might not be available
// We'll construct Account directly in the Installation conversion above

/// Represents a GitHub App installation.
///
/// This struct contains information about where a GitHub App is installed,
/// such as an organization or user account.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Installation {
    /// The unique ID of the installation
    pub id: u64,
    /// The account (user or organization) where the app is installed
    pub account: Account,
    /// Optional repository selection details
    pub repository_selection: Option<String>,
    /// The node ID for GraphQL operations
    pub node_id: String,
}

impl From<octocrab::models::Installation> for Installation {
    fn from(value: octocrab::models::Installation) -> Self {
        let account_node_id = value.account.node_id.clone();
        Self {
            id: *value.id,
            account: Account {
                id: *value.account.id,
                login: value.account.login,
                account_type: value.account.r#type,
                node_id: value.account.node_id,
            },
            repository_selection: value.repository_selection,
            node_id: account_node_id, // Use account's node_id since installation doesn't have one
        }
    }
}

/// Represents a label on a pull request.
///
/// This struct contains the essential information about a label
/// that is needed for categorization and filtering.
///
/// # Fields
///
/// * `name` - The name of the label
///
/// # Examples
///
/// ```
/// use release_regent_github_client::models::Label;
///
/// let label = Label {
///     name: "bug".to_string(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    /// The name of the label
    pub name: String,
}

/// Represents a GitHub organization.
///
/// This struct contains basic information about a GitHub organization,
/// primarily used for organization-related API operations and queries.
///
/// # Examples
///
/// ```rust
/// use release_regent_github_client::models::Organization;
///
/// let org = Organization {
///     name: "my-organization".to_string(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organization {
    /// The name of the organization
    pub name: String,
}

/// Represents a GitHub repository.
///
/// This struct contains essential information about a GitHub repository,
/// including its name, visibility, and identifiers. It provides methods
/// for accessing repository properties and generating URLs.
///
/// # Examples
///
/// ```rust
/// use release_regent_github_client::models::Repository;
///
/// let repo = Repository::new(
///     "my-repo".to_string(),
///     "owner/my-repo".to_string(),
///     "MDEwOlJlcG9zaXRvcnkx".to_string(),
///     false
/// );
///
/// println!("Repository: {}", repo.name());
/// println!("Is private: {}", repo.is_private());
/// println!("Clone URL: {}", repo.url());
/// ```
#[derive(Deserialize)]
pub struct Repository {
    /// The full name of the repository (owner/name)
    full_name: String,
    /// The name of the repository
    name: String,
    /// The GraphQL node ID of the repository
    node_id: String,
    /// Whether the repository is private
    private: bool,
}

impl Repository {
    /// Returns whether the repository is private.
    ///
    /// # Returns
    ///
    /// `true` if the repository is private, `false` if it's public.
    pub fn is_private(&self) -> bool {
        self.private
    }

    /// Returns the name of the repository.
    ///
    /// # Returns
    ///
    /// A string slice containing the repository name (without owner).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Creates a new Repository instance.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the repository
    /// * `full_name` - The full name including owner (owner/repo)
    /// * `node_id` - The GraphQL node ID
    /// * `private` - Whether the repository is private
    ///
    /// # Returns
    ///
    /// A new `Repository` instance with the provided values.
    pub fn new(name: String, full_name: String, node_id: String, private: bool) -> Self {
        Self {
            full_name,
            name,
            node_id,
            private,
        }
    }

    /// Returns the GraphQL node ID of the repository.
    ///
    /// # Returns
    ///
    /// A string slice containing the node ID used for GraphQL operations.
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// Returns the Git clone URL for the repository.
    ///
    /// # Returns
    ///
    /// A `Url` pointing to the Git clone endpoint for this repository.
    ///
    /// # Panics
    ///
    /// Panics if the repository full name cannot be formatted into a valid URL.
    /// This should not happen with valid GitHub repository names.
    pub fn url(&self) -> Url {
        Url::parse(&format!("https://github.com/{}.git", self.full_name))
            .expect("Valid GitHub repository URL")
    }
}

impl From<octocrab::models::Repository> for Repository {
    fn from(value: octocrab::models::Repository) -> Self {
        Self {
            name: value.name.clone(),
            full_name: value.full_name.unwrap_or(value.name.clone()),
            node_id: value.node_id.unwrap_or_default(),
            private: value.private.unwrap_or(false),
        }
    }
}

/// Represents a GitHub user account.
///
/// This struct contains basic information about a GitHub user, including
/// their unique ID and login name. It's used throughout the API for
/// representing repository owners, collaborators, and other user references.
///
/// # Examples
///
/// ```rust
/// use release_regent_github_client::models::User;
///
/// let user = User {
///     id: 12345,
///     login: "octocat".to_string(),
/// };
///
/// println!("User: {} (ID: {})", user.login, user.id);
/// ```
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct User {
    /// The unique numeric ID of the user
    pub id: u64,
    /// The login name of the user
    pub login: String,
}
