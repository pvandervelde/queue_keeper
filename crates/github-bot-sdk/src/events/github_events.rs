//! GitHub-specific event structures and types.
//!
//! This module defines typed structures for different GitHub webhook event types
//! including pull requests, issues, pushes, check runs, and check suites.

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::client::{Issue, PullRequest, Repository};

// ============================================================================
// Pull Request Events
// ============================================================================

/// Pull request event with action and details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequestEvent {
    /// Action that triggered this event
    pub action: PullRequestAction,

    /// Pull request number
    pub number: u32,

    /// Pull request details
    pub pull_request: PullRequest,

    /// Repository information
    pub repository: Repository,

    /// User who triggered the event
    pub sender: EventUser,
}

/// Actions that can occur on pull requests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullRequestAction {
    Opened,
    Closed,
    Reopened,
    Synchronize,
    Edited,
    Assigned,
    Unassigned,
    ReviewRequested,
    ReviewRequestRemoved,
    Labeled,
    Unlabeled,
    ReadyForReview,
    ConvertedToDraft,
}

impl fmt::Display for PullRequestAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Opened => "opened",
            Self::Closed => "closed",
            Self::Reopened => "reopened",
            Self::Synchronize => "synchronize",
            Self::Edited => "edited",
            Self::Assigned => "assigned",
            Self::Unassigned => "unassigned",
            Self::ReviewRequested => "review_requested",
            Self::ReviewRequestRemoved => "review_request_removed",
            Self::Labeled => "labeled",
            Self::Unlabeled => "unlabeled",
            Self::ReadyForReview => "ready_for_review",
            Self::ConvertedToDraft => "converted_to_draft",
        };
        write!(f, "{}", s)
    }
}

// ============================================================================
// Issue Events
// ============================================================================

/// Issue event with action and details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueEvent {
    /// Action that triggered this event
    pub action: IssueAction,

    /// Issue details
    pub issue: Issue,

    /// Repository information
    pub repository: Repository,

    /// User who triggered the event
    pub sender: EventUser,
}

/// Actions that can occur on issues.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueAction {
    Opened,
    Closed,
    Reopened,
    Edited,
    Assigned,
    Unassigned,
    Labeled,
    Unlabeled,
    Transferred,
    Pinned,
    Unpinned,
}

impl fmt::Display for IssueAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Opened => "opened",
            Self::Closed => "closed",
            Self::Reopened => "reopened",
            Self::Edited => "edited",
            Self::Assigned => "assigned",
            Self::Unassigned => "unassigned",
            Self::Labeled => "labeled",
            Self::Unlabeled => "unlabeled",
            Self::Transferred => "transferred",
            Self::Pinned => "pinned",
            Self::Unpinned => "unpinned",
        };
        write!(f, "{}", s)
    }
}

// ============================================================================
// Push Events
// ============================================================================

/// Push event with commit information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushEvent {
    /// Git ref that was pushed (e.g., "refs/heads/main")
    #[serde(rename = "ref")]
    pub ref_name: String,

    /// Commit SHA before the push
    pub before: String,

    /// Commit SHA after the push
    pub after: String,

    /// Whether this push created the ref
    pub created: bool,

    /// Whether this push deleted the ref
    pub deleted: bool,

    /// Whether this was a force push
    pub forced: bool,

    /// Commits included in the push
    pub commits: Vec<Commit>,

    /// The most recent commit
    pub head_commit: Option<Commit>,

    /// Repository information
    pub repository: Repository,

    /// User who triggered the event
    pub sender: EventUser,
}

/// Commit information from a push event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    /// Commit SHA
    pub id: String,

    /// Commit message
    pub message: String,

    /// Commit timestamp
    pub timestamp: String,

    /// Commit author
    pub author: CommitAuthor,

    /// Commit committer
    pub committer: CommitAuthor,
}

/// Author or committer information in a commit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitAuthor {
    /// Name
    pub name: String,

    /// Email address
    pub email: String,

    /// GitHub username (if available)
    pub username: Option<String>,
}

// ============================================================================
// Check Run Events
// ============================================================================

/// Check run event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckRunEvent {
    /// Action that triggered this event
    pub action: CheckRunAction,

    /// Check run details
    pub check_run: CheckRun,

    /// Repository information
    pub repository: Repository,

    /// User who triggered the event
    pub sender: EventUser,
}

/// Actions that can occur on check runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckRunAction {
    Created,
    Completed,
    Rerequested,
    RequestedAction,
}

/// Check run details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckRun {
    /// Check run ID
    pub id: u64,

    /// Check run name
    pub name: String,

    /// Check run status
    pub status: String,

    /// Check run conclusion (if completed)
    pub conclusion: Option<String>,
}

// ============================================================================
// Check Suite Events
// ============================================================================

/// Check suite event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckSuiteEvent {
    /// Action that triggered this event
    pub action: CheckSuiteAction,

    /// Check suite details
    pub check_suite: CheckSuite,

    /// Repository information
    pub repository: Repository,

    /// User who triggered the event
    pub sender: EventUser,
}

/// Actions that can occur on check suites.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckSuiteAction {
    Completed,
    Requested,
    Rerequested,
}

/// Check suite details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckSuite {
    /// Check suite ID
    pub id: u64,

    /// Check suite status
    pub status: String,

    /// Check suite conclusion (if completed)
    pub conclusion: Option<String>,
}

// ============================================================================
// Shared Types
// ============================================================================

/// User information in event payloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventUser {
    /// User ID
    pub id: u64,

    /// Username
    pub login: String,

    /// User type (User, Bot, Organization)
    #[serde(rename = "type")]
    pub user_type: String,
}

#[cfg(test)]
#[path = "github_events_tests.rs"]
mod tests;
