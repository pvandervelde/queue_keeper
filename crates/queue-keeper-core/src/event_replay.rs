// GENERATED FROM: specs/interfaces/event-replay.md
// Event Replay Interface - Administrative event reprocessing capabilities
//
// This module provides interfaces for replaying events from blob storage
// for debugging, recovery, and testing scenarios with proper ordering
// and idempotency guarantees.

use crate::{
    webhook::WrappedEvent, BotName, EventId, QueueName, Repository, SessionId, Timestamp,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt, str::FromStr, time::Duration};
use thiserror::Error;
use ulid::Ulid;

// ============================================================================
// Core Types
// ============================================================================

/// Request to replay events from storage
///
/// Supports various replay strategies: single event, session-based,
/// repository-based, or time-based ranges.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayRequest {
    /// Unique identifier for this replay operation
    pub replay_id: ReplayId,

    /// Type of replay operation
    pub replay_type: ReplayType,

    /// Who requested the replay (for audit)
    pub requester: String,

    /// Reason for replay (for audit)
    pub reason: String,

    /// Whether to respect original event ordering
    pub preserve_order: bool,

    /// Target bot queue filters (empty = all configured bots)
    pub target_bots: Vec<BotName>,

    /// Maximum number of events to replay (safety limit)
    pub max_events: Option<usize>,

    /// Dry run mode (validate but don't execute)
    pub dry_run: bool,

    /// When replay was requested
    pub requested_at: Timestamp,
}

impl ReplayRequest {
    /// Create replay request for single event
    pub fn single_event(event_id: EventId, requester: String, reason: String) -> Self {
        Self {
            replay_id: ReplayId::new(),
            replay_type: ReplayType::SingleEvent { event_id },
            requester,
            reason,
            preserve_order: false,
            target_bots: vec![],
            max_events: Some(1),
            dry_run: false,
            requested_at: Timestamp::now(),
        }
    }

    /// Create replay request for entire session
    pub fn session(session_id: SessionId, requester: String, reason: String) -> Self {
        Self {
            replay_id: ReplayId::new(),
            replay_type: ReplayType::Session { session_id },
            requester,
            reason,
            preserve_order: true, // Sessions require ordering
            target_bots: vec![],
            max_events: None,
            dry_run: false,
            requested_at: Timestamp::now(),
        }
    }

    /// Create replay request for repository events in time range
    pub fn repository_range(
        repository: Repository,
        start_time: Timestamp,
        end_time: Timestamp,
        requester: String,
        reason: String,
    ) -> Self {
        Self {
            replay_id: ReplayId::new(),
            replay_type: ReplayType::Repository {
                repository,
                start_time,
                end_time,
            },
            requester,
            reason,
            preserve_order: false,
            target_bots: vec![],
            max_events: None,
            dry_run: false,
            requested_at: Timestamp::now(),
        }
    }

    /// Validate replay request constraints
    pub fn validate(&self) -> Result<(), ReplayError> {
        // Validate requester
        if self.requester.is_empty() {
            return Err(ReplayError::InvalidRequest {
                reason: "Requester cannot be empty".to_string(),
            });
        }

        // Validate reason
        if self.reason.is_empty() {
            return Err(ReplayError::InvalidRequest {
                reason: "Reason cannot be empty".to_string(),
            });
        }

        // Validate max_events
        if let Some(max) = self.max_events {
            if max == 0 {
                return Err(ReplayError::InvalidRequest {
                    reason: "max_events must be greater than 0".to_string(),
                });
            }
        }

        // Validate replay type specific constraints
        self.replay_type.validate()?;

        Ok(())
    }
}

/// Type of event replay operation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplayType {
    /// Replay single event by ID
    SingleEvent { event_id: EventId },

    /// Replay all events for a session (maintains order)
    Session { session_id: SessionId },

    /// Replay events for specific repository in time range
    Repository {
        repository: Repository,
        start_time: Timestamp,
        end_time: Timestamp,
    },

    /// Replay events matching specific criteria
    Filtered {
        filter: EventFilter,
        start_time: Timestamp,
        end_time: Timestamp,
    },

    /// Replay failed events from dead letter queue
    DeadLetterQueue {
        queue_name: QueueName,
        failure_reason: Option<String>,
    },
}

impl ReplayType {
    /// Get estimated event count for this replay type
    pub async fn estimate_event_count(
        &self,
        _event_store: &dyn EventStore,
    ) -> Result<usize, ReplayError> {
        match self {
            ReplayType::SingleEvent { .. } => Ok(1),
            ReplayType::Session { session_id: _ } => {
                // TODO: Implement session event count estimation
                unimplemented!("Session event count estimation not yet implemented")
            }
            ReplayType::Repository {
                repository: _,
                start_time: _,
                end_time: _,
            } => {
                // TODO: Implement repository event count estimation
                unimplemented!("Repository event count estimation not yet implemented")
            }
            ReplayType::Filtered {
                filter: _,
                start_time: _,
                end_time: _,
            } => {
                // TODO: Implement filtered event count estimation
                unimplemented!("Filtered event count estimation not yet implemented")
            }
            ReplayType::DeadLetterQueue { .. } => {
                // TODO: Implement DLQ event count estimation
                unimplemented!("DLQ event count estimation not yet implemented")
            }
        }
    }

    /// Check if this replay type requires ordered processing
    pub fn requires_ordering(&self) -> bool {
        match self {
            ReplayType::SingleEvent { .. } => false,
            ReplayType::Session { .. } => true, // Sessions must maintain order
            ReplayType::Repository { .. } => false,
            ReplayType::Filtered { .. } => false,
            ReplayType::DeadLetterQueue { .. } => false,
        }
    }

    /// Get human-readable description
    pub fn description(&self) -> String {
        match self {
            ReplayType::SingleEvent { event_id } => {
                format!("Single event {}", event_id)
            }
            ReplayType::Session { session_id } => {
                format!("Session {}", session_id)
            }
            ReplayType::Repository {
                repository,
                start_time,
                end_time,
            } => {
                format!(
                    "Repository {} from {} to {}",
                    repository.full_name, start_time, end_time
                )
            }
            ReplayType::Filtered {
                filter: _,
                start_time,
                end_time,
            } => {
                format!("Filtered events from {} to {}", start_time, end_time)
            }
            ReplayType::DeadLetterQueue {
                queue_name,
                failure_reason,
            } => match failure_reason {
                Some(reason) => format!("DLQ {} ({})", queue_name, reason),
                None => format!("DLQ {}", queue_name),
            },
        }
    }

    /// Validate replay type constraints
    pub fn validate(&self) -> Result<(), ReplayError> {
        match self {
            ReplayType::Repository {
                start_time,
                end_time,
                ..
            }
            | ReplayType::Filtered {
                start_time,
                end_time,
                ..
            } => {
                if end_time <= start_time {
                    return Err(ReplayError::InvalidRequest {
                        reason: "end_time must be after start_time".to_string(),
                    });
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// Filter criteria for event selection
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EventFilter {
    /// Event types to include
    pub event_types: Option<Vec<String>>,

    /// Repositories to include
    pub repositories: Option<Vec<Repository>>,

    /// Session IDs to include
    pub session_ids: Option<Vec<SessionId>>,

    /// Original processing status
    pub processing_status: Option<ProcessingStatus>,

    /// Bot routing results
    pub routing_results: Option<RoutingResultFilter>,
}

impl EventFilter {
    /// Check if event matches this filter
    pub fn matches(&self, event: &StoredEvent) -> bool {
        // Check event types
        if let Some(ref types) = self.event_types {
            if !types.contains(&event.envelope.event_type) {
                return false;
            }
        }

        // Check repositories
        if let Some(ref repos) = self.repositories {
            if let Some(repo) = event
                .envelope
                .payload
                .get("repository")
                .and_then(|r| serde_json::from_value::<Repository>(r.clone()).ok())
            {
                if !repos.contains(&repo) {
                    return false;
                }
            } else {
                // No repository info - cannot match repository filter
                return false;
            }
        }

        // Check session IDs
        if let Some(ref sessions) = self.session_ids {
            if !event
                .envelope
                .session_id
                .as_ref()
                .map_or(false, |s| sessions.contains(s))
            {
                return false;
            }
        }

        // TODO: Check processing status and routing results

        true
    }

    /// Create filter for failed events only
    pub fn failed_events() -> Self {
        Self {
            processing_status: Some(ProcessingStatus::Failed),
            ..Default::default()
        }
    }

    /// Create filter for specific event types
    pub fn event_types(types: Vec<String>) -> Self {
        Self {
            event_types: Some(types),
            ..Default::default()
        }
    }

    /// Create filter for specific repositories
    pub fn repositories(repos: Vec<Repository>) -> Self {
        Self {
            repositories: Some(repos),
            ..Default::default()
        }
    }
}

/// Processing status filter
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessingStatus {
    /// Successfully processed and routed
    Success,

    /// Failed during processing
    Failed,

    /// Partially processed (some bots succeeded, some failed)
    Partial,
}

/// Bot routing result filter
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoutingResultFilter {
    /// Events that were routed to specific bot
    RoutedToBot(BotName),

    /// Events that failed routing to specific bot
    FailedRoutingToBot(BotName),

    /// Events with no matching bot subscriptions
    NoMatchingBots,

    /// Events routed to any bot
    RoutedToAnyBot,
}

/// Unique identifier for replay operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReplayId(Ulid);

impl ReplayId {
    /// Generate new replay ID
    pub fn new() -> Self {
        Self(Ulid::new())
    }
}

impl Default for ReplayId {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplayId {
    /// Get string representation
    pub fn as_str(&self) -> String {
        self.0.to_string()
    }

    /// Get underlying ULID
    pub fn as_ulid(&self) -> Ulid {
        self.0
    }
}

impl fmt::Display for ReplayId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for ReplayId {
    type Err = ReplayError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ulid = s.parse().map_err(|_| ReplayError::InvalidReplayId {
            replay_id: s.to_string(),
        })?;
        Ok(Self(ulid))
    }
}

// ============================================================================
// Status and Progress Types
// ============================================================================

/// Current status of a replay operation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplayStatus {
    /// Replay operation identifier
    pub replay_id: ReplayId,

    /// Current state of the replay
    pub state: ReplayState,

    /// Progress information
    pub progress: ReplayProgress,

    /// Any errors encountered
    pub errors: Vec<ReplayError>,

    /// When replay started
    pub started_at: Timestamp,

    /// When replay completed (if finished)
    pub completed_at: Option<Timestamp>,

    /// Execution statistics
    pub statistics: ReplayStatistics,
}

impl ReplayStatus {
    /// Check if replay is currently active
    pub fn is_active(&self) -> bool {
        matches!(
            self.state,
            ReplayState::Pending
                | ReplayState::LoadingEvents
                | ReplayState::ValidatingEvents
                | ReplayState::Replaying
        )
    }

    /// Check if replay completed successfully
    pub fn is_successful(&self) -> bool {
        matches!(self.state, ReplayState::Completed)
    }

    /// Get completion percentage (0.0 to 1.0)
    pub fn completion_percentage(&self) -> f64 {
        if self.progress.total_events == 0 {
            return 0.0;
        }

        let completed = self.progress.events_completed
            + self.progress.events_failed
            + self.progress.events_skipped;
        completed as f64 / self.progress.total_events as f64
    }

    /// Get estimated time remaining
    pub fn estimated_time_remaining(&self) -> Option<Duration> {
        if !self.is_active() || self.progress.processing_rate <= 0.0 {
            return None;
        }

        let remaining_events = self.progress.total_events.saturating_sub(
            self.progress.events_completed
                + self.progress.events_failed
                + self.progress.events_skipped,
        );

        if remaining_events == 0 {
            return Some(Duration::from_secs(0));
        }

        let estimated_seconds = remaining_events as f64 / self.progress.processing_rate;
        Some(Duration::from_secs_f64(estimated_seconds))
    }
}

/// State of replay operation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplayState {
    /// Replay is queued but not started
    Pending,

    /// Currently loading events from storage
    LoadingEvents,

    /// Validating events before replay
    ValidatingEvents,

    /// Actively replaying events
    Replaying,

    /// Replay completed successfully
    Completed,

    /// Replay failed with errors
    Failed,

    /// Replay was cancelled by user
    Cancelled,
}

/// Progress tracking for replay operations
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplayProgress {
    /// Total events to replay
    pub total_events: usize,

    /// Events successfully replayed
    pub events_completed: usize,

    /// Events that failed replay
    pub events_failed: usize,

    /// Events skipped (duplicates, filters, etc.)
    pub events_skipped: usize,

    /// Current event being processed
    pub current_event: Option<EventId>,

    /// Processing rate (events per second)
    pub processing_rate: f64,
}

impl Default for ReplayProgress {
    fn default() -> Self {
        Self {
            total_events: 0,
            events_completed: 0,
            events_failed: 0,
            events_skipped: 0,
            current_event: None,
            processing_rate: 0.0,
        }
    }
}

/// Statistics for completed replay operations
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplayStatistics {
    /// Total execution time
    pub total_duration: Duration,

    /// Time spent loading events
    pub load_duration: Duration,

    /// Time spent replaying events
    pub replay_duration: Duration,

    /// Average processing time per event
    pub avg_event_processing_time: Duration,

    /// Events routed by bot
    pub events_by_bot: HashMap<BotName, usize>,

    /// Duplicate events detected and skipped
    pub duplicates_detected: usize,

    /// Network/service errors encountered
    pub transient_errors: usize,
}

impl Default for ReplayStatistics {
    fn default() -> Self {
        Self {
            total_duration: Duration::from_secs(0),
            load_duration: Duration::from_secs(0),
            replay_duration: Duration::from_secs(0),
            avg_event_processing_time: Duration::from_secs(0),
            events_by_bot: HashMap::new(),
            duplicates_detected: 0,
            transient_errors: 0,
        }
    }
}

// ============================================================================
// Core Interfaces
// ============================================================================

/// Interface for event replay operations
///
/// Provides administrative capabilities to replay events from blob storage
/// with proper ordering, filtering, and safety constraints.
#[async_trait]
pub trait EventReplayService: Send + Sync {
    /// Submit replay request for processing
    ///
    /// Validates request, estimates scope, and queues for execution.
    /// Returns immediately with replay ID for status tracking.
    async fn submit_replay(&self, request: ReplayRequest) -> Result<ReplayId, ReplayError>;

    /// Get status of ongoing or completed replay
    async fn get_replay_status(&self, replay_id: ReplayId) -> Result<ReplayStatus, ReplayError>;

    /// List all replay operations (with optional filters)
    async fn list_replays(
        &self,
        filter: Option<ReplayListFilter>,
    ) -> Result<Vec<ReplayStatus>, ReplayError>;

    /// Cancel ongoing replay operation
    async fn cancel_replay(
        &self,
        replay_id: ReplayId,
        requester: String,
    ) -> Result<(), ReplayError>;

    /// Get detailed replay results
    async fn get_replay_results(&self, replay_id: ReplayId) -> Result<ReplayResults, ReplayError>;

    /// Validate replay request without executing
    async fn validate_replay_request(
        &self,
        request: &ReplayRequest,
    ) -> Result<ReplayEstimate, ReplayError>;

    /// Get replay service health and capacity
    async fn get_service_status(&self) -> Result<ReplayServiceStatus, ReplayError>;
}

/// Interface for retrieving stored events for replay
///
/// Abstracts event storage access to support different storage backends
/// and provide optimized queries for replay operations.
#[async_trait]
pub trait EventRetriever: Send + Sync {
    /// Retrieve single event by ID
    async fn get_event(&self, event_id: EventId) -> Result<Option<StoredEvent>, ReplayError>;

    /// Retrieve all events for a session (in chronological order)
    async fn get_session_events(
        &self,
        session_id: SessionId,
    ) -> Result<Vec<StoredEvent>, ReplayError>;

    /// Retrieve events matching filter criteria
    async fn get_events_by_filter(
        &self,
        filter: EventFilter,
        start_time: Timestamp,
        end_time: Timestamp,
        limit: Option<usize>,
    ) -> Result<Vec<StoredEvent>, ReplayError>;

    /// Retrieve events from time range for repository
    async fn get_repository_events(
        &self,
        repository: &Repository,
        start_time: Timestamp,
        end_time: Timestamp,
        limit: Option<usize>,
    ) -> Result<Vec<StoredEvent>, ReplayError>;

    /// Count events matching criteria (for estimation)
    async fn count_events_by_filter(
        &self,
        filter: &EventFilter,
        start_time: Timestamp,
        end_time: Timestamp,
    ) -> Result<usize, ReplayError>;

    /// Check if event has been processed recently (duplicate detection)
    async fn is_recent_duplicate(
        &self,
        event_id: EventId,
        window: Duration,
    ) -> Result<bool, ReplayError>;
}

/// Interface for executing event replay operations
///
/// Handles the actual reprocessing of events through the webhook pipeline
/// with proper ordering, error handling, and progress tracking.
#[async_trait]
pub trait ReplayExecutor: Send + Sync {
    /// Execute replay of events
    ///
    /// Processes events through the normal webhook pipeline with
    /// duplicate detection and progress reporting.
    async fn execute_replay(
        &self,
        replay_id: ReplayId,
        events: Vec<StoredEvent>,
        options: ReplayExecutionOptions,
    ) -> Result<ReplayResults, ReplayError>;

    /// Execute single event replay
    async fn replay_single_event(
        &self,
        event: StoredEvent,
        options: ReplayExecutionOptions,
    ) -> Result<EventReplayResult, ReplayError>;

    /// Validate events before replay (dry run)
    async fn validate_events(
        &self,
        events: &[StoredEvent],
        options: &ReplayExecutionOptions,
    ) -> Result<ReplayValidationResult, ReplayError>;

    /// Get current execution capacity
    async fn get_execution_capacity(&self) -> Result<ExecutionCapacity, ReplayError>;
}

// ============================================================================
// Supporting Types
// ============================================================================

/// Event retrieved from blob storage for replay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredEvent {
    /// Original wrapped event
    pub envelope: WrappedEvent,

    /// Storage metadata
    pub storage_metadata: StorageMetadata,

    /// Original webhook payload
    pub raw_payload: Option<Vec<u8>>,

    /// Processing history (if available)
    pub processing_history: Option<ProcessingHistory>,
}

impl StoredEvent {
    /// Check if event is valid for replay
    pub fn is_replayable(&self) -> bool {
        // Basic validation - event has required fields
        !self.envelope.event_id.to_string().is_empty()
            && !self.envelope.event_type.is_empty()
            && self.storage_metadata.signature_valid
    }

    /// Get event age
    pub fn get_age(&self) -> Duration {
        let now = Timestamp::now();
        now.duration_since(self.envelope.received_at)
    }

    /// Check if event has been replayed recently
    pub fn has_recent_replay(&self, window: Duration) -> bool {
        if let Some(ref history) = self.processing_history {
            let cutoff = Timestamp::now().subtract_duration(window);
            history
                .replay_attempts
                .iter()
                .any(|attempt| attempt.attempted_at > cutoff)
        } else {
            false
        }
    }
}

/// Metadata from blob storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageMetadata {
    /// Blob storage path
    pub blob_path: String,

    /// When event was stored
    pub stored_at: Timestamp,

    /// Storage content hash
    pub content_hash: String,

    /// Original webhook signature status
    pub signature_valid: bool,

    /// Blob size in bytes
    pub size_bytes: u64,
}

/// Processing history for event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingHistory {
    /// Original processing attempts
    pub original_processing: Vec<ProcessingAttempt>,

    /// Previous replay attempts
    pub replay_attempts: Vec<ReplayAttempt>,

    /// Last known routing results
    pub last_routing_results: Vec<RoutingResult>,
}

/// Individual processing attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingAttempt {
    /// When processing was attempted
    pub attempted_at: Timestamp,

    /// Processing result
    pub result: ProcessingResult,

    /// Bots that received the event
    pub routed_bots: Vec<BotName>,

    /// Any errors encountered
    pub errors: Vec<String>,
}

/// Previous replay attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayAttempt {
    /// Replay operation ID
    pub replay_id: ReplayId,

    /// When replay was attempted
    pub attempted_at: Timestamp,

    /// Who requested the replay
    pub requester: String,

    /// Replay result
    pub result: EventReplayResult,
}

/// Processing result
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessingResult {
    /// Successfully processed
    Success,

    /// Failed to process
    Failed,

    /// Partially processed
    Partial,
}

/// Bot routing result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingResult {
    /// Bot that received (or should have received) the event
    pub bot_name: BotName,

    /// Whether routing was successful
    pub success: bool,

    /// Error message if routing failed
    pub error: Option<String>,

    /// When routing was attempted
    pub attempted_at: Timestamp,
}

// ============================================================================
// Configuration and Options
// ============================================================================

/// Configuration for replay operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayConfiguration {
    /// Maximum concurrent replay operations
    pub max_concurrent_replays: usize,

    /// Maximum events per replay operation
    pub max_events_per_replay: usize,

    /// Duplicate detection window
    pub duplicate_detection_window: Duration,

    /// Event processing timeout
    pub event_timeout: Duration,

    /// Batch size for event processing
    pub batch_size: usize,

    /// Enable detailed progress tracking
    pub track_detailed_progress: bool,

    /// Retain replay results for this duration
    pub result_retention_duration: Duration,
}

impl Default for ReplayConfiguration {
    fn default() -> Self {
        Self {
            max_concurrent_replays: 5,
            max_events_per_replay: 10000,
            duplicate_detection_window: Duration::from_secs(3600), // 1 hour
            event_timeout: Duration::from_secs(30),
            batch_size: 100,
            track_detailed_progress: true,
            result_retention_duration: Duration::from_secs(7 * 24 * 3600), // 1 week
        }
    }
}

/// Options for replay execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayExecutionOptions {
    /// Target bots for replay (empty = all configured)
    pub target_bots: Vec<BotName>,

    /// Skip duplicate detection
    pub skip_duplicate_check: bool,

    /// Preserve original event ordering
    pub preserve_order: bool,

    /// Stop on first error
    pub fail_fast: bool,

    /// Progress reporting interval
    pub progress_report_interval: Duration,

    /// Batch processing options
    pub batch_options: BatchOptions,
}

impl Default for ReplayExecutionOptions {
    fn default() -> Self {
        Self {
            target_bots: vec![],
            skip_duplicate_check: false,
            preserve_order: false,
            fail_fast: false,
            progress_report_interval: Duration::from_secs(5),
            batch_options: BatchOptions::default(),
        }
    }
}

/// Batch processing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchOptions {
    /// Number of events per batch
    pub batch_size: usize,

    /// Delay between batches
    pub batch_delay: Duration,

    /// Maximum concurrent batches
    pub max_concurrent_batches: usize,
}

impl Default for BatchOptions {
    fn default() -> Self {
        Self {
            batch_size: 100,
            batch_delay: Duration::from_millis(100),
            max_concurrent_batches: 3,
        }
    }
}

// ============================================================================
// Result Types
// ============================================================================

/// Results from completed replay operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayResults {
    /// Replay operation ID
    pub replay_id: ReplayId,

    /// Final status
    pub final_status: ReplayState,

    /// Overall statistics
    pub statistics: ReplayStatistics,

    /// Per-event results
    pub event_results: Vec<EventReplayResult>,

    /// Summary of routing results
    pub routing_summary: RoutingSummary,

    /// Any errors that occurred
    pub errors: Vec<ReplayError>,

    /// Execution metadata
    pub execution_metadata: ExecutionMetadata,
}

/// Result for individual event replay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventReplayResult {
    /// Event that was replayed
    pub event_id: EventId,

    /// Replay outcome
    pub result: ReplayOutcome,

    /// Bots that received the event
    pub routed_bots: Vec<BotName>,

    /// Processing time
    pub processing_time: Duration,

    /// Any errors encountered
    pub errors: Vec<String>,

    /// Whether this was a duplicate
    pub was_duplicate: bool,
}

/// Outcome of event replay
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplayOutcome {
    /// Event successfully replayed
    Success,

    /// Event skipped (duplicate, filtered, etc.)
    Skipped { reason: String },

    /// Event failed to replay
    Failed { error: String },

    /// Event partially replayed (some bots succeeded)
    Partial {
        successful_bots: Vec<BotName>,
        failed_bots: Vec<BotName>,
    },
}

/// Summary of routing results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingSummary {
    /// Events routed by bot
    pub events_by_bot: HashMap<BotName, usize>,

    /// Events with no matching bots
    pub unrouted_events: usize,

    /// Routing errors encountered
    pub routing_errors: Vec<String>,

    /// New bot subscriptions discovered
    pub new_bot_matches: HashMap<BotName, usize>,
}

/// Execution metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMetadata {
    /// Executor instance that handled replay
    pub executor_instance: String,

    /// Configuration used during replay
    pub replay_config: ReplayConfiguration,

    /// Resource usage during replay
    pub resource_usage: ResourceUsage,

    /// Concurrency and batching info
    pub execution_strategy: ExecutionStrategy,
}

/// Resource usage tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// Peak memory usage in bytes
    pub peak_memory_bytes: u64,

    /// CPU time used
    pub cpu_time: Duration,

    /// Network bytes transferred
    pub network_bytes: u64,

    /// Storage operations performed
    pub storage_operations: usize,
}

/// Execution strategy info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStrategy {
    /// Batch size used
    pub batch_size: usize,

    /// Concurrent operations
    pub concurrency_level: usize,

    /// Ordering strategy
    pub ordering_strategy: String,

    /// Retry strategy
    pub retry_strategy: String,
}

// ============================================================================
// Additional Types
// ============================================================================

/// Filter for listing replay operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayListFilter {
    /// Filter by state
    pub state: Option<ReplayState>,

    /// Filter by requester
    pub requester: Option<String>,

    /// Filter by time range
    pub started_after: Option<Timestamp>,

    /// Filter by time range
    pub started_before: Option<Timestamp>,

    /// Limit results
    pub limit: Option<usize>,
}

/// Estimate for replay operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayEstimate {
    /// Estimated number of events
    pub estimated_events: usize,

    /// Estimated duration
    pub estimated_duration: Duration,

    /// Estimated resource usage
    pub estimated_resources: ResourceEstimate,

    /// Any warnings or concerns
    pub warnings: Vec<String>,
}

/// Resource usage estimate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceEstimate {
    /// Estimated memory usage
    pub memory_mb: u64,

    /// Estimated network transfer
    pub network_mb: u64,

    /// Estimated storage operations
    pub storage_operations: usize,
}

/// Service status for replay service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayServiceStatus {
    /// Current active replays
    pub active_replays: usize,

    /// Maximum concurrent replays
    pub max_replays: usize,

    /// Service health
    pub healthy: bool,

    /// Current capacity utilization (0.0 to 1.0)
    pub capacity_utilization: f64,

    /// Pending replay requests
    pub pending_requests: usize,
}

/// Execution capacity information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionCapacity {
    /// Available executor threads
    pub available_threads: usize,

    /// Maximum events per second
    pub max_events_per_second: f64,

    /// Current load (0.0 to 1.0)
    pub current_load: f64,

    /// Can accept new replay
    pub can_accept_new: bool,
}

/// Validation result for events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayValidationResult {
    /// Total events validated
    pub total_events: usize,

    /// Valid events
    pub valid_events: usize,

    /// Invalid/skipped events
    pub invalid_events: usize,

    /// Validation errors
    pub validation_errors: Vec<String>,

    /// Estimated processing time
    pub estimated_processing_time: Duration,
}

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during event replay operations
#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplayError {
    #[error("Replay not found: {replay_id}")]
    ReplayNotFound { replay_id: ReplayId },

    #[error("Invalid replay ID: {replay_id}")]
    InvalidReplayId { replay_id: String },

    #[error("Event not found: {event_id}")]
    EventNotFound { event_id: EventId },

    #[error("Invalid replay request: {reason}")]
    InvalidRequest { reason: String },

    #[error("Replay capacity exceeded: {current_replays}/{max_replays}")]
    CapacityExceeded {
        current_replays: usize,
        max_replays: usize,
    },

    #[error("Replay operation cancelled by {requester}")]
    Cancelled { requester: String },

    #[error("Event retrieval failed: {message}")]
    EventRetrievalFailed { message: String },

    #[error("Event processing failed: {event_id} - {message}")]
    EventProcessingFailed { event_id: EventId, message: String },

    #[error("Storage access failed: {message}")]
    StorageError { message: String },

    #[error("Configuration error: {message}")]
    Configuration { message: String },

    #[error("Service unavailable: {service} - {message}")]
    ServiceUnavailable { service: String, message: String },

    #[error("Timeout during {operation} after {duration:?}")]
    Timeout {
        operation: String,
        duration: Duration,
    },

    #[error("Internal error: {message}")]
    Internal { message: String },
}

impl ReplayError {
    /// Check if error is transient and replay might succeed later
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            ReplayError::ServiceUnavailable { .. }
                | ReplayError::Timeout { .. }
                | ReplayError::Internal { .. }
        )
    }

    /// Check if error indicates a configuration problem
    pub fn is_configuration_error(&self) -> bool {
        matches!(
            self,
            ReplayError::InvalidRequest { .. } | ReplayError::Configuration { .. }
        )
    }

    /// Get retry delay for transient errors
    pub fn get_retry_delay(&self) -> Option<Duration> {
        match self {
            ReplayError::ServiceUnavailable { .. } => Some(Duration::from_secs(30)),
            ReplayError::Timeout { .. } => Some(Duration::from_secs(10)),
            _ => None,
        }
    }
}

// ============================================================================
// Stub Traits (TODO: Move to appropriate modules)
// ============================================================================

/// Stub trait for event storage (TODO: Move to appropriate module)
#[async_trait]
pub trait EventStore: Send + Sync {
    /// Count events (stub)
    async fn count_events(&self) -> Result<usize, ReplayError>;
}

// ============================================================================
// Default Implementations (Stubs)
// ============================================================================

/// Default implementation of EventReplayService
pub struct DefaultEventReplayService;

#[async_trait]
impl EventReplayService for DefaultEventReplayService {
    async fn submit_replay(&self, _request: ReplayRequest) -> Result<ReplayId, ReplayError> {
        unimplemented!(
            "Event replay service not yet implemented - see specs/interfaces/event-replay.md"
        )
    }

    async fn get_replay_status(&self, _replay_id: ReplayId) -> Result<ReplayStatus, ReplayError> {
        unimplemented!(
            "Event replay service not yet implemented - see specs/interfaces/event-replay.md"
        )
    }

    async fn list_replays(
        &self,
        _filter: Option<ReplayListFilter>,
    ) -> Result<Vec<ReplayStatus>, ReplayError> {
        unimplemented!(
            "Event replay service not yet implemented - see specs/interfaces/event-replay.md"
        )
    }

    async fn cancel_replay(
        &self,
        _replay_id: ReplayId,
        _requester: String,
    ) -> Result<(), ReplayError> {
        unimplemented!(
            "Event replay service not yet implemented - see specs/interfaces/event-replay.md"
        )
    }

    async fn get_replay_results(&self, _replay_id: ReplayId) -> Result<ReplayResults, ReplayError> {
        unimplemented!(
            "Event replay service not yet implemented - see specs/interfaces/event-replay.md"
        )
    }

    async fn validate_replay_request(
        &self,
        _request: &ReplayRequest,
    ) -> Result<ReplayEstimate, ReplayError> {
        unimplemented!(
            "Event replay service not yet implemented - see specs/interfaces/event-replay.md"
        )
    }

    async fn get_service_status(&self) -> Result<ReplayServiceStatus, ReplayError> {
        unimplemented!(
            "Event replay service not yet implemented - see specs/interfaces/event-replay.md"
        )
    }
}

/// Default implementation of EventRetriever
pub struct DefaultEventRetriever;

#[async_trait]
impl EventRetriever for DefaultEventRetriever {
    async fn get_event(&self, _event_id: EventId) -> Result<Option<StoredEvent>, ReplayError> {
        unimplemented!("Event retriever not yet implemented - see specs/interfaces/event-replay.md")
    }

    async fn get_session_events(
        &self,
        _session_id: SessionId,
    ) -> Result<Vec<StoredEvent>, ReplayError> {
        unimplemented!("Event retriever not yet implemented - see specs/interfaces/event-replay.md")
    }

    async fn get_events_by_filter(
        &self,
        _filter: EventFilter,
        _start_time: Timestamp,
        _end_time: Timestamp,
        _limit: Option<usize>,
    ) -> Result<Vec<StoredEvent>, ReplayError> {
        unimplemented!("Event retriever not yet implemented - see specs/interfaces/event-replay.md")
    }

    async fn get_repository_events(
        &self,
        _repository: &Repository,
        _start_time: Timestamp,
        _end_time: Timestamp,
        _limit: Option<usize>,
    ) -> Result<Vec<StoredEvent>, ReplayError> {
        unimplemented!("Event retriever not yet implemented - see specs/interfaces/event-replay.md")
    }

    async fn count_events_by_filter(
        &self,
        _filter: &EventFilter,
        _start_time: Timestamp,
        _end_time: Timestamp,
    ) -> Result<usize, ReplayError> {
        unimplemented!("Event retriever not yet implemented - see specs/interfaces/event-replay.md")
    }

    async fn is_recent_duplicate(
        &self,
        _event_id: EventId,
        _window: Duration,
    ) -> Result<bool, ReplayError> {
        unimplemented!("Event retriever not yet implemented - see specs/interfaces/event-replay.md")
    }
}

/// Default implementation of ReplayExecutor
pub struct DefaultReplayExecutor;

#[async_trait]
impl ReplayExecutor for DefaultReplayExecutor {
    async fn execute_replay(
        &self,
        _replay_id: ReplayId,
        _events: Vec<StoredEvent>,
        _options: ReplayExecutionOptions,
    ) -> Result<ReplayResults, ReplayError> {
        unimplemented!("Replay executor not yet implemented - see specs/interfaces/event-replay.md")
    }

    async fn replay_single_event(
        &self,
        _event: StoredEvent,
        _options: ReplayExecutionOptions,
    ) -> Result<EventReplayResult, ReplayError> {
        unimplemented!("Replay executor not yet implemented - see specs/interfaces/event-replay.md")
    }

    async fn validate_events(
        &self,
        _events: &[StoredEvent],
        _options: &ReplayExecutionOptions,
    ) -> Result<ReplayValidationResult, ReplayError> {
        unimplemented!("Replay executor not yet implemented - see specs/interfaces/event-replay.md")
    }

    async fn get_execution_capacity(&self) -> Result<ExecutionCapacity, ReplayError> {
        unimplemented!("Replay executor not yet implemented - see specs/interfaces/event-replay.md")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replay_id_creation() {
        let id1 = ReplayId::new();
        let id2 = ReplayId::new();

        assert_ne!(id1, id2);
        assert!(!id1.as_str().is_empty());
    }

    #[test]
    fn test_replay_request_validation() {
        let request = ReplayRequest::single_event(
            EventId::new(),
            "test-user".to_string(),
            "test reason".to_string(),
        );

        assert!(request.validate().is_ok());

        let invalid_request = ReplayRequest {
            requester: "".to_string(),
            ..request
        };

        assert!(invalid_request.validate().is_err());
    }

    #[test]
    fn test_replay_status_progress() {
        let status = ReplayStatus {
            replay_id: ReplayId::new(),
            state: ReplayState::Replaying,
            progress: ReplayProgress {
                total_events: 100,
                events_completed: 25,
                events_failed: 5,
                events_skipped: 10,
                ..Default::default()
            },
            errors: vec![],
            started_at: Timestamp::now(),
            completed_at: None,
            statistics: ReplayStatistics::default(),
        };

        assert!(status.is_active());
        assert!(!status.is_successful());
        assert_eq!(status.completion_percentage(), 0.4); // (25+5+10)/100
    }

    #[test]
    fn test_event_filter_matching() {
        let filter = EventFilter::event_types(vec!["push".to_string(), "pull_request".to_string()]);

        let event = StoredEvent {
            envelope: WrappedEvent::new(
                "github".to_string(),
                "push".to_string(),
                None,
                Some(SessionId::from_parts("owner", "repo", "push", "event1")),
                serde_json::json!({
                    "repository": {
                        "id": 123,
                        "name": "repo",
                        "full_name": "owner/repo",
                        "private": false,
                        "owner": {"id": 456, "login": "owner", "type": "User"}
                    }
                }),
            ),
            storage_metadata: StorageMetadata {
                blob_path: "path".to_string(),
                stored_at: Timestamp::now(),
                content_hash: "hash".to_string(),
                signature_valid: true,
                size_bytes: 1024,
            },
            raw_payload: None,
            processing_history: None,
        };

        assert!(filter.matches(&event));

        let mut non_matching_event = event.clone();
        non_matching_event.envelope.event_type = "issues".to_string();
        assert!(!filter.matches(&non_matching_event));
    }
}
