//! GitHub App metadata types.
//!
//! This module contains types representing GitHub App information returned
//! from the GitHub API.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::auth::User;

/// GitHub App metadata.
///
/// Represents a GitHub App's configuration and metadata as returned by
/// the GitHub API's `/app` endpoint.
///
/// # Examples
///
/// ```no_run
/// # use github_bot_sdk::client::GitHubClient;
/// # use github_bot_sdk::auth::AuthenticationProvider;
/// # async fn example(client: &GitHubClient) -> Result<(), Box<dyn std::error::Error>> {
/// let app = client.get_app().await?;
/// println!("App: {} (ID: {})", app.name, app.id);
/// println!("Owner: {}", app.owner.login);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct App {
    /// Unique numeric identifier for the GitHub App
    pub id: u64,

    /// URL-friendly string identifier for the app
    pub slug: String,

    /// Display name of the GitHub App
    pub name: String,

    /// Owner of the GitHub App (user or organization)
    pub owner: User,

    /// Description of the app's purpose
    pub description: Option<String>,

    /// External URL for the app (e.g., homepage)
    pub external_url: String,

    /// GitHub URL for the app's page
    pub html_url: String,

    /// When the app was created
    pub created_at: DateTime<Utc>,

    /// When the app was last updated
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
