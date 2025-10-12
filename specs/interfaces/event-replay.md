# Event Replay Interface

**Architectural Layer**: Core Domain
**Module Path**: `src/event_replay.rs`
**Responsibilities** (from RDD):

- Knows: Event storage locations, replay constraints, ordering requirements
- Does: Orchestrates event reprocessing, validates replay requests, maintains audit trail

## Dependencies

- Types: `EventId`, `EventEnvelope`, `ReplayRequest`, `BlobReference` (event-replay-types.md)
- Interfaces: `BlobStorage`, `QueueRouter`, `EventStore` (blob-storage.md, bot-configuration.md)
- Shared: `Result<T, E>`, `Timestamp`, `SessionId` (shared-types.md)

## Overview

The Event Replay Interface defines how Queue-Keeper reprocesses events from blob storage for debugging, recovery, and testing scenarios. This system implements REQ-008 (Replay Capabilities) by providing administrative interfaces to replay individual events, event ranges, or entire sessions with proper ordering and idempotency guarantees.

**Critical Design Principles:**

- **Idempotency**: Duplicate detection via event IDs prevents reprocessing
- **Order Preservation**: Session-based replays maintain chronological order
- **Audit Trail**: All replay operations logged with requester and reason
- **Selective Replay**: Replay by event ID, session, repository, or time range
- **Safety Constraints**: Configurable limits to prevent system overload

## Types

### ReplayRequest

Request specification for event replay operations.

```rust
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
    pub fn single_event(
        event_id: EventId,
        requester: String,
        reason: String,
    ) -> Self;

    /// Create replay request for entire session
    pub fn session(
        session_id: SessionId,
        requester: String,
        reason: String,
    ) -> Self;

    /// Create replay request for repository events in time range
    pub fn repository_range(
        repository: Repository,
        start_time: Timestamp,
        end_time: Timestamp,
        requester: String,
        reason: String,
    ) -> Self;

    /// Validate replay request constraints
    pub fn validate(&self) -> Result<(), ReplayError>;
}
```

### ReplayType

Different types of replay operations supported.

```rust
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
    pub async fn estimate_event_count(&self, event_store: &dyn EventStore) -> Result<usize, ReplayError>;

    /// Check if this replay type requires ordered processing
    pub fn requires_ordering(&self) -> bool;

    /// Get human-readable description
    pub fn description(&self) -> String;
}
```

### EventFilter

Filter criteria for selective event replay.

```rust
/// Filter criteria for event selection
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    pub fn matches(&self, event: &StoredEvent) -> bool;

    /// Create filter for failed events only
    pub fn failed_events() -> Self;

    /// Create filter for specific event types
    pub fn event_types(types: Vec<String>) -> Self;

    /// Create filter for specific repositories
    pub fn repositories(repos: Vec<Repository>) -> Self;
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
```

### ReplayStatus

Status tracking for replay operations.

```rust
/// Current status of a replay operation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    pub fn is_active(&self) -> bool;

    /// Check if replay completed successfully
    pub fn is_successful(&self) -> bool;

    /// Get completion percentage (0.0 to 1.0)
    pub fn completion_percentage(&self) -> f64;

    /// Get estimated time remaining
    pub fn estimated_time_remaining(&self) -> Option<Duration>;
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// Statistics for completed replay operations
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
```

### ReplayId

Unique identifier for replay operations.

```rust
/// Unique identifier for replay operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReplayId(Ulid);

impl ReplayId {
    /// Generate new replay ID
    pub fn new() -> Self {
        Self(Ulid::new())
    }

    /// Parse from string
    pub fn from_str(s: &str) -> Result<Self, ReplayError> {
        let ulid = s.parse().map_err(|_| ReplayError::InvalidReplayId {
            replay_id: s.to_string()
        })?;
        Ok(Self(ulid))
    }

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
```

## Core Interfaces

### EventReplayService

Main interface for event replay operations.

```rust
/// Interface for event replay operations
///
/// Provides administrative capabilities to replay events from blob storage
/// with proper ordering, filtering, and safety constraints.
#[async_trait::async_trait]
pub trait EventReplayService: Send + Sync {
    /// Submit replay request for processing
    ///
    /// Validates request, estimates scope, and queues for execution.
    /// Returns immediately with replay ID for status tracking.
    async fn submit_replay(&self, request: ReplayRequest) -> Result<ReplayId, ReplayError>;

    /// Get status of ongoing or completed replay
    async fn get_replay_status(&self, replay_id: ReplayId) -> Result<ReplayStatus, ReplayError>;

    /// List all replay operations (with optional filters)
    async fn list_replays(&self, filter: Option<ReplayListFilter>) -> Result<Vec<ReplayStatus>, ReplayError>;

    /// Cancel ongoing replay operation
    async fn cancel_replay(&self, replay_id: ReplayId, requester: String) -> Result<(), ReplayError>;

    /// Get detailed replay results
    async fn get_replay_results(&self, replay_id: ReplayId) -> Result<ReplayResults, ReplayError>;

    /// Validate replay request without executing
    async fn validate_replay_request(&self, request: &ReplayRequest) -> Result<ReplayEstimate, ReplayError>;

    /// Get replay service health and capacity
    async fn get_service_status(&self) -> Result<ReplayServiceStatus, ReplayError>;
}
```

### EventRetriever

Interface for retrieving events from storage for replay.

```rust
/// Interface for retrieving stored events for replay
///
/// Abstracts event storage access to support different storage backends
/// and provide optimized queries for replay operations.
#[async_trait::async_trait]
pub trait EventRetriever: Send + Sync {
    /// Retrieve single event by ID
    async fn get_event(&self, event_id: EventId) -> Result<Option<StoredEvent>, ReplayError>;

    /// Retrieve all events for a session (in chronological order)
    async fn get_session_events(&self, session_id: SessionId) -> Result<Vec<StoredEvent>, ReplayError>;

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
    async fn is_recent_duplicate(&self, event_id: EventId, window: Duration) -> Result<bool, ReplayError>;
}
```

### ReplayExecutor

Interface for executing event replay operations.

```rust
/// Interface for executing event replay operations
///
/// Handles the actual reprocessing of events through the webhook pipeline
/// with proper ordering, error handling, and progress tracking.
#[async_trait::async_trait]
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
```

## Supporting Types

### StoredEvent

Event as retrieved from blob storage with metadata.

```rust
/// Event retrieved from blob storage for replay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredEvent {
    /// Original event envelope
    pub envelope: EventEnvelope,

    /// Storage metadata
    pub storage_metadata: StorageMetadata,

    /// Original webhook payload
    pub raw_payload: Option<Vec<u8>>,

    /// Processing history (if available)
    pub processing_history: Option<ProcessingHistory>,
}

impl StoredEvent {
    /// Check if event is valid for replay
    pub fn is_replayable(&self) -> bool;

    /// Get event age
    pub fn get_age(&self) -> Duration;

    /// Check if event has been replayed recently
    pub fn has_recent_replay(&self, window: Duration) -> bool;
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
```

### ReplayResults

Results and statistics from completed replay operations.

```rust
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
    Partial { successful_bots: Vec<BotName>, failed_bots: Vec<BotName> },
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
```

### Configuration Types

Configuration and options for replay operations.

```rust
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
```

## Error Types

### ReplayError

Comprehensive error type for replay operations.

```rust
/// Errors that can occur during event replay operations
#[derive(Debug, thiserror::Error)]
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
    CapacityExceeded { current_replays: usize, max_replays: usize },

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
    Timeout { operation: String, duration: Duration },

    #[error("Internal error: {message}")]
    Internal { message: String },
}

impl ReplayError {
    /// Check if error is transient and replay might succeed later
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            ReplayError::ServiceUnavailable { .. } |
            ReplayError::Timeout { .. } |
            ReplayError::Internal { .. }
        )
    }

    /// Check if error indicates a configuration problem
    pub fn is_configuration_error(&self) -> bool {
        matches!(
            self,
            ReplayError::InvalidRequest { .. } |
            ReplayError::Configuration { .. }
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
```

## Usage Examples

### Single Event Replay

```rust
// Replay single event for debugging
let replay_request = ReplayRequest::single_event(
    event_id,
    "admin@company.com".to_string(),
    "Debugging webhook processing issue #123".to_string(),
);

let replay_id = replay_service.submit_replay(replay_request).await?;

// Monitor progress
loop {
    let status = replay_service.get_replay_status(replay_id).await?;

    match status.state {
        ReplayState::Completed => {
            let results = replay_service.get_replay_results(replay_id).await?;
            println!("Replay completed: {} events processed", results.statistics.total_events);
            break;
        }
        ReplayState::Failed => {
            println!("Replay failed: {:?}", status.errors);
            break;
        }
        _ => {
            println!("Progress: {:.1}%", status.completion_percentage() * 100.0);
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }
}
```

### Session Replay

```rust
// Replay entire session to fix ordering issues
let replay_request = ReplayRequest::session(
    session_id,
    "ops-team".to_string(),
    "Reprocessing PR #456 events in correct order".to_string(),
);

// Enable ordering preservation for session replays
replay_request.preserve_order = true;

let replay_id = replay_service.submit_replay(replay_request).await?;
```

### Filtered Replay

```rust
// Replay failed events from last 24 hours
let filter = EventFilter {
    processing_status: Some(ProcessingStatus::Failed),
    event_types: Some(vec!["pull_request.opened".to_string(), "issues.opened".to_string()]),
    ..Default::default()
};

let replay_request = ReplayRequest {
    replay_id: ReplayId::new(),
    replay_type: ReplayType::Filtered {
        filter,
        start_time: Timestamp::now().subtract_duration(Duration::from_secs(24 * 3600)),
        end_time: Timestamp::now(),
    },
    requester: "automated-recovery".to_string(),
    reason: "Daily failed event recovery".to_string(),
    preserve_order: false,
    target_bots: vec![], // All bots
    max_events: Some(1000),
    dry_run: false,
    requested_at: Timestamp::now(),
};
```

## Implementation Considerations

### Performance

1. **Batch Processing**: Process events in configurable batches to balance throughput and memory usage
2. **Concurrent Execution**: Support multiple concurrent replay operations with capacity limits
3. **Progress Tracking**: Efficient progress updates without impacting replay performance
4. **Memory Management**: Stream large event sets instead of loading everything into memory

### Safety

1. **Duplicate Detection**: Prevent reprocessing events that were recently processed
2. **Rate Limiting**: Limit replay throughput to avoid overwhelming downstream systems
3. **Capacity Management**: Queue replay requests when system capacity is exceeded
4. **Timeout Handling**: Proper timeouts for all external operations

### Observability

1. **Comprehensive Logging**: Log all replay operations with correlation IDs
2. **Metrics**: Track replay success rates, throughput, and error patterns
3. **Audit Trail**: Maintain complete audit trail for compliance and debugging
4. **Progress Monitoring**: Real-time progress tracking for long-running replays

### Error Handling

1. **Transient Retry**: Automatic retry for transient failures with exponential backoff
2. **Partial Success**: Continue processing remaining events when some fail
3. **Error Aggregation**: Collect and report all errors from batch operations
4. **Graceful Degradation**: Continue operation when non-critical services are unavailable

This Event Replay interface provides comprehensive support for REQ-008 while ensuring operational safety, proper audit trails, and robust error handling for production environments.
