//! GitHub App authentication module.
//!
//! This module provides comprehensive GitHub App authentication functionality including
//! JWT generation, installation token management, rate limiting, and secure token storage.
//!
//! # Architecture
//!
//! The module is built around the following core components:
//!
//! * `GitHubAuthManager` - Central authentication coordinator
//! * `TokenCache` - Secure in-memory token storage with automatic cleanup
//! * `AuthConfig` - Configuration for GitHub App settings and Enterprise support
//! * `RateLimiter` - Rate limiting for authentication endpoints
//!
//! # Security Features
//!
//! * Secure token storage using `secrecy` crate
//! * Automatic token cleanup on drop
//! * No sensitive data in error messages or logs
//! * Constant-time comparisons for signature verification
//!
//! # Examples
//!
//! ```rust,no_run
//! use release_regent_github_client::auth::{GitHubAuthManager, AuthConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = AuthConfig::new(
//!         12345,
//!         "-----BEGIN RSA PRIVATE KEY-----\n...\n-----END RSA PRIVATE KEY-----",
//!         None, // GitHub.com (not Enterprise)
//!     )?;
//!
//!     let auth_manager = GitHubAuthManager::new(config)?;
//!     let token = auth_manager.get_installation_token(987654).await?;
//!
//!     println!("Got installation token for installation ID 987654");
//!     Ok(())
//! }
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey};
use octocrab::Octocrab;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument};

use crate::errors::{Error, GitHubResult};
