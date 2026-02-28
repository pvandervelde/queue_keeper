# Queue-Keeper Configuration Guide

## Overview

Queue-Keeper uses static YAML configuration files to define bot subscriptions and event routing rules. Configuration is loaded at startup and remains immutable at runtime, requiring a container restart for any changes. This approach ensures configuration consistency and simplifies deployment.

## Quick Start

### Minimal Configuration Example

Create a `bot-config.yaml` file:

```yaml
bots:
  - name: "task-tactician"
    queue: "queue-keeper-task-tactician"
    events: ["issues.opened", "issues.closed", "issues.labeled"]
    ordered: true

  - name: "merge-warden"
    queue: "queue-keeper-merge-warden"
    events: ["pull_request.opened", "pull_request.synchronize"]
    ordered: true

  - name: "notification-bot"
    queue: "queue-keeper-notifications"
    events: ["*"]  # All events
    ordered: false
```

### Loading Configuration

**From File:**

```bash
export BOT_CONFIG_PATH=/path/to/bot-config.yaml
cargo run --package queue-keeper-service
```

**From Environment Variable:**

```bash
export BOT_CONFIGURATION='{"bots": [{"name": "my-bot", "queue": "my-queue", "events": ["issues.*"], "ordered": true}]}'
cargo run --package queue-keeper-service
```

**In Container:**

```bash
docker run -p 8080:8080 \
  -v $(pwd)/bot-config.yaml:/config/bot-config.yaml:ro \
  -e BOT_CONFIG_PATH=/config/bot-config.yaml \
  ghcr.io/pvandervelde/queue-keeper:latest
```

## Configuration Schema

### Bot Subscription Structure

Each bot subscription defines how events should be routed to a specific downstream service.

```yaml
bots:
  - name: string              # Required: Unique bot identifier
    queue: string             # Required: Target Azure Service Bus queue name
    events: [string]          # Required: GitHub event types to subscribe to
    ordered: boolean          # Required: Whether to use session-based ordering
    repository_filter:        # Optional: Filter events by repository (YAML tag format)
      !exact                  # Use !exact tag for specific repository
      owner: string           # Repository owner (organization or user)
      name: string            # Repository name
    config:                   # Optional: Bot-specific configuration
      settings:               # Required wrapper for bot configuration
        key: value            # Custom key-value pairs passed to bot
```

### Required Fields

#### `name` (string)

- Unique identifier for the bot
- Used in logging, metrics, and debugging
- Must be 1-64 characters
- Allowed characters: letters, numbers, hyphens
- Must not start or end with hyphen
- Must not contain consecutive hyphens (`--`)
- Example: `"task-tactician"`, `"merge-warden"`

#### `queue` (string)

- Target Azure Service Bus queue name where events will be sent
- Must start with `queue-keeper-` prefix
- Must follow Azure Service Bus naming conventions:
  - 1-260 characters
  - Only letters, numbers, periods (.), hyphens (-), underscores (_)
  - Must begin and end with letter or number
- Example: `"queue-keeper-task-tactician"`

#### `events` (array of strings)

- List of GitHub event types this bot subscribes to
- Supports multiple pattern formats (see Event Pattern Syntax below)
- At least one event pattern required
- Example: `["issues.opened", "issues.closed"]`

#### `ordered` (boolean)

- `true`: Events delivered with session-based ordering (FIFO)
- `false`: Events delivered in parallel without ordering guarantees
- See Ordering and Sessions section for details

### Optional Fields

#### `repository_filter` (object)

Filter events to only specific repositories. Supports multiple filter types:

**Single Repository:**

```yaml
repository_filter:
  !exact
  owner: "myorg"
  name: "myrepo"
```

**Multiple Repositories (OR logic):**

```yaml
repository_filter:
  !any_of
  - !exact
    owner: "myorg"
    name: "repo1"
  - !exact
    owner: "myorg"
    name: "repo2"
  - !exact
    owner: "anotherorg"
    name: "repo3"
```

**All Repositories from Organization:**

```yaml
repository_filter: !owner myorg
```

**Pattern Matching:**

```yaml
repository_filter: !name_pattern ^prod-.*  # Regex: repositories starting with "prod-"
```

**Complex Filters (AND logic):**

```yaml
repository_filter:
  !all_of
  - !owner myorg
  - !name_pattern .*-service$  # Repos ending with "-service"
```

When specified, only events from matching repositories will be routed to this bot.

#### `config` (object)

Bot-specific configuration passed along with each event:

```yaml
config:
  settings:
    priority: "high"
    timeout_seconds: 300
    custom_setting: "value"
```

These key-value pairs are included in the event envelope and available to the bot for custom behavior.

## Event Pattern Syntax

Queue-Keeper supports flexible event matching patterns:

### Exact Match

```yaml
events: ["issues.opened"]
```

Matches only `issues.opened` events.

### Wildcard Match

```yaml
events: ["issues.*"]
```

Matches all issue-related events: `issues.opened`, `issues.closed`, `issues.labeled`, etc.

### Multiple Patterns

```yaml
events:
  - "issues.opened"
  - "issues.closed"
  - "pull_request.*"
```

Matches any of the specified patterns.

### Exclusion Pattern

```yaml
events:
  - "issues.*"           # All issue events
  - "!issues.deleted"    # Except deletions
```

Excludes specific event types by prefixing with `!`. Useful for subscribing to broad patterns while excluding specific events. Exclusions are processed after inclusions.

### All Events

```yaml
events: ["*"]
```

Matches all GitHub webhook events. Use cautiously as this includes high-volume events.

### Event Type Reference

Common GitHub webhook event types:

**Issues:**

- `issues.opened`, `issues.closed`, `issues.reopened`
- `issues.labeled`, `issues.unlabeled`
- `issues.assigned`, `issues.unassigned`
- `issues.edited`, `issues.deleted`

**Pull Requests:**

- `pull_request.opened`, `pull_request.closed`, `pull_request.reopened`
- `pull_request.synchronize` (new commits pushed)
- `pull_request.labeled`, `pull_request.unlabeled`
- `pull_request.assigned`, `pull_request.review_requested`
- `pull_request.edited`

**Other Common Events:**

- `push` (commits pushed to branch)
- `release.published`, `release.created`
- `workflow_run.completed`
- `deployment.created`, `deployment_status.created`

See [GitHub Webhook Events](https://docs.github.com/en/webhooks/webhook-events-and-payloads) for complete list.

## Ordering and Sessions

### When to Use Ordering

**Use `ordered: true` when:**

- Bot maintains state for entities (issues, PRs)
- Processing order affects correctness
- Events must be processed sequentially per entity
- Example: Task management bot tracking issue lifecycle

**Use `ordered: false` when:**

- Bot is stateless (notifications, logging)
- Events can be processed independently
- Maximum throughput is priority
- Example: Notification bot, metrics collector

### How Ordering Works

When `ordered: true`:

1. **Session ID Generation**: Queue-Keeper generates a session ID based on the entity:
   - For issues: `{owner}/{repo}/issue/{issue_number}`
   - For PRs: `{owner}/{repo}/pull_request/{pr_number}`
   - For repository events: `{owner}/{repo}/repository/repository`
   - For branch events: `{owner}/{repo}/branch/{branch_name}`
   - For release events: `{owner}/{repo}/release/{tag}`

2. **Session-Based Delivery**: Events with the same session ID are delivered in order
3. **Concurrent Processing**: Different sessions can be processed in parallel
4. **Maximum Sessions**: Configure `max_concurrent_sessions` to control concurrency

### Ordering Configuration Example

```yaml
bots:
  - name: "state-tracking-bot"
    queue: "queue-keeper-state-tracker"
    events: ["issues.*", "pull_request.*"]
    ordered: true

  - name: "notification-bot"
    queue: "queue-keeper-notifications"
    events: ["*"]
    ordered: false  # No ordering needed, maximize throughput
```

## Validation

Queue-Keeper validates configuration at startup and fails fast if errors are detected.

### Validation Rules

**Bot Names:**

- Must be unique across all bots
- 1-50 characters
- Only letters, numbers, hyphens, underscores

**Queue Names:**

- Must start with `queue-keeper-` prefix
- Must follow Azure Service Bus naming rules
- 1-260 characters
- Valid characters: letters, numbers, `.`, `-`, `_`
- Must start and end with letter or number

**Event Patterns:**

- Should match GitHub webhook event types (not enforced at startup)
- Wildcards allowed with `*`
- At least one event pattern per bot

**Ordering Consistency:**

- Bots with `ordered: true` must have valid session configuration
- Repository filters with `!exact` tag must specify both owner and name

### Validation Errors

Example validation error output:

```
Configuration validation failed:
  - Bot "task-tactician": Invalid queue name "invalid/queue" (contains invalid character '/')
  - Bot "merge-warden": Duplicate bot name
  - Bot "notification-bot": Invalid event pattern "invalid_event" (unknown event type)
```

Fix errors and restart the service to apply corrected configuration.

## Advanced Configuration

### Multiple Bots for Same Events

Multiple bots can subscribe to the same events (fan-out pattern):

```yaml
bots:
  - name: "task-manager"
    queue: "queue-keeper-task-manager"
    events: ["issues.opened"]
    ordered: true

  - name: "notifier"
    queue: "queue-keeper-notifications"
    events: ["issues.opened"]
    ordered: false

  - name: "metrics"
    queue: "queue-keeper-metrics"
    events: ["issues.opened"]
    ordered: false
```

When an `issues.opened` event arrives, Queue-Keeper delivers it to all three queues.

### Repository-Specific Bots

Route events from specific repositories to dedicated bots:

**Single Repository:**

```yaml
bots:
  - name: "production-watcher"
    queue: "queue-keeper-prod-watcher"
    events: ["push"]
    ordered: true
    repository_filter:
      !exact
      owner: "myorg"
      name: "production-app"
```

**Multiple Repositories:**

```yaml
bots:
  - name: "critical-repos-monitor"
    queue: "queue-keeper-critical-monitor"
    events: ["pull_request.*", "push"]
    ordered: true
    repository_filter:
      !any_of
      - !exact
        owner: "myorg"
        name: "production-app"
      - !exact
        owner: "myorg"
        name: "customer-api"
      - !exact
        owner: "myorg"
        name: "payment-service"
```

**All Repositories from Organization:**

```yaml
bots:
  - name: "org-wide-monitor"
    queue: "queue-keeper-org-monitor"
    events: ["issues.*"]
    ordered: false
    repository_filter: !owner myorg  # All repos owned by "myorg"
```

**Pattern-Based Filtering:**

```yaml
bots:
  - name: "service-repos-monitor"
    queue: "queue-keeper-services"
    events: ["deployment.*"]
    ordered: true
    repository_filter:
      !all_of
      - !owner myorg
      - !name_pattern .*-service$  # Only repos ending with "-service"
```

**No Filter (All Repositories):**

```yaml
bots:
  - name: "general-monitor"
    queue: "queue-keeper-monitor"
    events: ["push"]
    ordered: false
    # No repository_filter - receives push events from all repositories
```

### Bot-Specific Configuration

Pass custom configuration to bots:

```yaml
bots:
  - name: "custom-bot"
    queue: "queue-keeper-custom"
    events: ["issues.*"]
    ordered: true
    config:
      settings:
        priority: "high"
        retry_limit: 5
        timeout_ms: 30000
        labels_to_watch: ["bug", "critical"]
        assignee_required: true
```

The `config.settings` object is included in the event envelope payload sent to the bot's queue.

## Service Configuration File

The HTTP service itself is configured by a separate YAML file (`service.yaml`),
distinct from the bot-subscription configuration described above.

### Loading Sources (Priority Order)

Configuration is merged from these sources in ascending priority order (later
sources override earlier ones):

| Priority | Source | Notes |
|----------|--------|-------|
| 1 (lowest) | `/etc/queue-keeper/service.yaml` | System-wide defaults |
| 2 | `./config/service.yaml` | Deployment-local override |
| 3 | Path from `QK_CONFIG_FILE` env var | Operator-specified file (required when set) |
| 4 (highest) | Environment variables with `QK__` prefix | Override any file value |

**Environment variable format:** double underscores (`__`) separate nesting
levels. For example:

```bash
QK__SERVER__PORT=9090          # sets server.port
QK__SERVER__HOST=127.0.0.1     # sets server.host
QK__LOGGING__LEVEL=debug       # sets logging.level
```

### Minimal Service Configuration

```yaml
# config/service.yaml
server:
  host: "0.0.0.0"
  port: 8080
```

### Full Service Configuration Schema

```yaml
server:
  host: "0.0.0.0"       # Bind address
  port: 8080             # Listen port

webhooks:
  max_payload_size: 26214400   # Max body in bytes (25 MB)
  timeout_seconds: 30          # Processing timeout

security:
  require_https: false         # Enforce TLS (true in production)
  allowed_origins: []          # CORS origins (empty = all)

logging:
  level: "info"                # trace | debug | info | warn | error
  format: "json"               # json | text

providers: []         # Standard GitHub webhook providers (see below)
generic_providers: [] # Configuration-driven generic providers (see below)
```

---

### `providers` — Standard GitHub Webhook Providers

Each entry in `providers` registers a GitHub-style webhook provider at
`POST /webhook/{id}`.

```yaml
providers:
  - id: "github"               # URL segment: POST /webhook/github
    require_signature: true    # Require HMAC-SHA256 X-Hub-Signature-256 header
    secret:
      type: key_vault          # or "literal" for development only
      secret_name: "github-webhook-secret"   # Azure Key Vault secret name
    allowed_event_types: []    # Empty = all event types accepted
```

#### Secret Sources for `providers`

| `type` | Description | Recommendation |
|--------|-------------|----------------|
| `key_vault` | Azure Key Vault secret | **Use in production** |
| `literal` | Hard-coded value in config | Development / CI only |

**Development example (literal secret):**

```yaml
providers:
  - id: "github"
    require_signature: true
    secret:
      type: literal
      value: "my-dev-secret"   # Never commit to source control
```

---

### `generic_providers` — Configuration-Driven Providers

Generic providers allow you to add non-GitHub webhook sources without
writing any Rust code. Each entry is registered at `POST /webhook/{provider_id}`.

#### Processing Modes

| Mode | Description |
|------|-------------|
| `direct` | Forward raw request body to a configured Azure Service Bus queue. |
| `wrap` | Parse JSON payload, extract fields, produce a standard `WrappedEvent`. |

#### Example: Direct Mode (Jira)

Forwards raw Jira webhook JSON to `queue-keeper-jira` without transformation:

```yaml
generic_providers:
  - provider_id: "jira"
    processing_mode: direct
    target_queue: "queue-keeper-jira"       # Required for direct mode
    event_type_source:
      source: header
      name: "X-Atlassian-Event"             # Use Jira's event header
    signature:
      header_name: "X-Hub-Signature"
      algorithm: hmac_sha256
    webhook_secret:
      type: literal
      value: "jira-dev-secret"              # Use key_vault in production
```

#### Example: Wrap Mode (GitLab)

Normalises GitLab webhooks into the standard `WrappedEvent` format and routes
them to bots using the standard subscription rules:

```yaml
generic_providers:
  - provider_id: "gitlab"
    processing_mode: wrap
    # Wrap mode does not need target_queue; routing uses BotConfiguration
    event_type_source:
      source: header
      name: "X-Gitlab-Event"
    signature:
      header_name: "X-Gitlab-Token"
      algorithm: bearer_token              # GitLab uses a shared token, not HMAC
    webhook_secret:
      type: key_vault
      secret_name: "gitlab-webhook-token"  # Retrieve from Key Vault
    field_extraction:
      repository_path: "project.path_with_namespace"   # "owner/repo"
      entity_path: "object_attributes.iid"             # MR/issue number
      action_path: "object_attributes.action"          # "open", "merge", etc.
```

#### Example: Wrap Mode (Slack Events API)

```yaml
generic_providers:
  - provider_id: "slack"
    processing_mode: wrap
    event_type_source:
      source: json_path
      path: "event.type"                    # e.g. "message", "app_mention"
    delivery_id_source:
      source: auto_generate                 # No Slack delivery ID header
    # No signature configured — rely on Slack's URL verification challenge
    field_extraction:
      repository_path: "team_id"            # Slack workspace ID as "repo"
      entity_path: "event.channel"
      action_path: "event.subtype"
```

#### `generic_providers` Field Reference

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `provider_id` | string | ✅ | URL-safe ID (`[a-z0-9-_]+`). Must be unique. |
| `processing_mode` | `direct` \| `wrap` | ✅ | How to process incoming payloads. |
| `target_queue` | string | Direct only | Azure Service Bus queue name. |
| `event_type_source` | FieldSource | No | Where to read the event type. Defaults to `"webhook"`. |
| `delivery_id_source` | FieldSource | No | Where to read the delivery ID. Defaults to auto-generated UUID. |
| `signature` | SignatureConfig | No | Signature validation settings. |
| `webhook_secret` | WebhookSecretConfig | No | Secret source for signature validation. |
| `field_extraction` | FieldExtractionConfig | Wrap only | JSON field paths for wrap-mode extraction. |

#### `FieldSource` Reference

| `source` | Additional fields | Description |
|----------|-------------------|-------------|
| `header` | `name: string` | Read from HTTP header (case-insensitive) |
| `json_path` | `path: string` | Dot-separated path into JSON body |
| `static` | `value: string` | Constant compile-time value |
| `auto_generate` | — | Auto-generated server-side value |

#### `SignatureConfig` Reference

| Field | Description |
|-------|-------------|
| `header_name` | HTTP header carrying the signature (e.g. `X-Hub-Signature-256`) |
| `algorithm` | `hmac_sha256` \| `hmac_sha1` \| `bearer_token` |

#### `WebhookSecretConfig` Reference

| `type` | Additional fields | Description |
|--------|-------------------|-------------|
| `key_vault` | `secret_name: string` | Azure Key Vault secret name — **use in production** |
| `literal` | `value: string` | Hard-coded secret — **development / CI only** |

---

## Environment Variables

### Configuration Loading

| Variable | Description | Example |
|----------|-------------|---------|
| `BOT_CONFIG_PATH` | Path to bot-subscription YAML file | `/config/bot-config.yaml` |
| `BOT_CONFIGURATION` | JSON bot-subscription string | `'{"bots": [...]}` |
| `QK_CONFIG_FILE` | Path to service configuration YAML | `/config/service.yaml` |

If both `BOT_CONFIG_PATH` and `BOT_CONFIGURATION` are set, `BOT_CONFIG_PATH` takes precedence.



### Global Settings

Queue-Keeper supports top-level configuration settings that control global behavior:

```yaml
settings:
  max_bots: 50                    # Maximum concurrent bot subscriptions
  default_message_ttl: 86400      # Default message TTL in seconds (24 hours)
  validate_on_startup: true       # Validate configuration at startup
  log_configuration: true         # Log configuration details on startup

bots:
  - name: "my-bot"
    # ... bot configuration
```

| Setting | Default | Description |
|---------|---------|-------------|
| `max_bots` | 50 | Maximum number of bot subscriptions allowed |
| `default_message_ttl` | 86400 | Default time-to-live for queue messages in seconds |
| `validate_on_startup` | true | Whether to validate configuration at startup |
| `log_configuration` | true | Whether to log configuration details at startup |

All settings are optional and use the default values shown if not specified.

### Service Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `QUEUE_KEEPER_LOG_LEVEL` | `info` | Log level (trace, debug, info, warn, error) |
| `QUEUE_KEEPER_PORT` | `8080` | HTTP server port |
| `QUEUE_KEEPER_HOST` | `0.0.0.0` | HTTP server bind address |

### Azure Integration

| Variable | Description |
|----------|-------------|
| `AZURE_SERVICE_BUS_NAMESPACE` | Azure Service Bus namespace |
| `AZURE_CLIENT_ID` | Managed Identity client ID |
| `AZURE_TENANT_ID` | Azure AD tenant ID |
| `GITHUB_WEBHOOK_SECRET` | GitHub webhook HMAC secret |

In production, secrets should be retrieved from Azure Key Vault automatically via Managed Identity.

## Container Deployment

### Kubernetes ConfigMap

Define configuration as a ConfigMap:

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: queue-keeper-config
  namespace: automation
data:
  bot-config.yaml: |
    bots:
      - name: "task-tactician"
        queue: "queue-keeper-task-tactician"
        events: ["issues.*"]
        ordered: true
      - name: "merge-warden"
        queue: "queue-keeper-merge-warden"
        events: ["pull_request.*"]
        ordered: true
```

Mount in deployment:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: queue-keeper
spec:
  template:
    spec:
      containers:
      - name: queue-keeper
        image: ghcr.io/pvandervelde/queue-keeper:latest
        env:
        - name: BOT_CONFIG_PATH
          value: /config/bot-config.yaml
        volumeMounts:
        - name: config
          mountPath: /config
          readOnly: true
      volumes:
      - name: config
        configMap:
          name: queue-keeper-config
```

### Azure Container Apps

Use Azure Container Apps environment variables and file mounts:

```bash
az containerapp create \
  --name queue-keeper \
  --resource-group mygroup \
  --environment myenv \
  --image ghcr.io/pvandervelde/queue-keeper:latest \
  --target-port 8080 \
  --env-vars \
    BOT_CONFIG_PATH=/config/bot-config.yaml \
    QUEUE_KEEPER_LOG_LEVEL=info \
  --cpu 0.5 \
  --memory 1Gi
```

## Configuration Updates

### Update Process

Since configuration is immutable at runtime:

1. **Update Configuration File**: Edit your `bot-config.yaml`
2. **Validate Changes**: Review validation errors from startup logs
3. **Update ConfigMap**: `kubectl apply -f configmap.yaml`
4. **Restart Service**: Rolling restart to load new configuration
5. **Verify**: Check logs for successful configuration load

### Zero-Downtime Updates

For production systems:

1. **Blue-Green Deployment**: Deploy new version with updated config
2. **Canary Testing**: Route small percentage of traffic to new config
3. **Health Checks**: Verify new configuration validates successfully
4. **Gradual Rollout**: Shift traffic to new version
5. **Rollback**: Keep previous version available for quick rollback

## Troubleshooting

### Configuration Not Loading

**Check file path:**

```bash
docker exec queue-keeper ls -la /config/bot-config.yaml
```

**Check environment variable:**

```bash
docker exec queue-keeper env | grep BOT_CONFIG
```

**Check logs:**

```bash
docker logs queue-keeper 2>&1 | grep -i config
```

### Validation Failures

**Common errors:**

1. **Duplicate bot names**: Ensure all bot names are unique
2. **Invalid queue names**: Check Azure Service Bus naming rules
3. **Unknown event types**: Verify against GitHub webhook documentation
4. **Malformed YAML**: Use YAML validator (yamllint)

**Validate YAML syntax:**

```bash
yamllint bot-config.yaml
```

### Events Not Routing

**Check configuration:**

- Verify event patterns match incoming webhook event types
- Check repository filters aren't excluding events
- Confirm queue names match actual Service Bus queues

**Check logs:**

```bash
# Look for routing decisions
docker logs queue-keeper 2>&1 | grep -i routing

# Check for delivery errors
docker logs queue-keeper 2>&1 | grep -i "delivery failed"
```

## Best Practices

### Configuration Management

1. **Version Control**: Store configuration in Git
2. **Environment Separation**: Separate configs for dev/staging/prod
3. **Secret Management**: Never store secrets in configuration files
4. **Validation**: Always validate before deploying
5. **Documentation**: Comment complex routing rules

### Event Subscription Design

1. **Be Specific**: Subscribe to specific events, not wildcards, when possible
2. **Use Filtering**: Apply repository filters to reduce noise
3. **Order When Needed**: Only use `ordered: true` when necessary
4. **Monitor Volume**: Track event volume per bot for capacity planning

### Performance Tuning

1. **Concurrent Sessions**: Tune `max_concurrent_sessions` for ordered bots
2. **Unordered for High Volume**: Use `ordered: false` for metrics/logging
3. **Repository Filters**: Reduce processing overhead with specific filters
4. **Multiple Instances**: Scale horizontally for high webhook volumes

## Examples

### Pull Requests for Multiple Repositories

Monitor pull request events for a specific set of repositories:

```yaml
bots:
  - name: "pr-reviewer-bot"
    queue: "queue-keeper-pr-reviewer"
    events:
      - "pull_request.opened"
      - "pull_request.synchronize"
      - "pull_request.review_requested"
    ordered: true
    repository_filter:
      !any_of
      - !exact
        owner: "myorg"
        name: "backend-api"
      - !exact
        owner: "myorg"
        name: "frontend-app"
      - !exact
        owner: "myorg"
        name: "mobile-app"
    config:
      settings:
        auto_assign_reviewers: true
        require_tests: true
```

### Complete Production Configuration

```yaml
# Production bot configuration
# Version: 1.0
# Last updated: 2026-02-05

bots:
  # Task management bot - tracks issue lifecycle
  - name: "task-tactician"
    queue: "queue-keeper-task-tactician"
    events:
      - "issues.opened"
      - "issues.closed"
      - "issues.labeled"
      - "issues.assigned"
    ordered: true
    config:
      settings:
        priority: "high"

  # PR management bot - handles merge workflows
  - name: "merge-warden"
    queue: "queue-keeper-merge-warden"
    events:
      - "pull_request.opened"
      - "pull_request.synchronize"
      - "pull_request.closed"
      - "pull_request.review_requested"
    ordered: true

  # Specification validator - checks PR changes
  - name: "spec-sentinel"
    queue: "queue-keeper-spec-sentinel"
    events:
      - "pull_request.opened"
      - "pull_request.synchronize"
    ordered: false
    config:
      validate_on_push: true

  # Production deployment monitor - critical repositories only
  - name: "prod-deploy-monitor"
    queue: "queue-keeper-prod-monitor"
    events:
      - "push"
      - "deployment.created"
      - "deployment_status.created"
    ordered: true
    repository_filter:
      !exact
      owner: "myorg"
      name: "production-app"
    config:
      settings:
        priority: "critical"
        alert_on_failure: true

  # General notification bot - all events, no ordering
  - name: "notification-hub"
    queue: "queue-keeper-notifications"
    events: ["*"]
    ordered: false
    config:
      settings:
        channels: ["slack", "email"]
        priority: "low"
```

## Additional Resources

- [Architecture Documentation](../specs/README.md) - System design and architecture
- [Container Usage Guide](./container-usage.md) - Container deployment details
- [API Documentation](https://docs.rs/queue-keeper-core) - Rustdoc API reference
- [GitHub Webhooks](https://docs.github.com/en/webhooks) - GitHub webhook documentation
- [Azure Service Bus](https://learn.microsoft.com/azure/service-bus-messaging/) - Queue provider documentation

## Support

For issues or questions:

- Open an issue: [GitHub Issues](https://github.com/pvandervelde/queue_keeper/issues)
- Review specifications: `specs/` directory
- Check examples: `examples/` directory (if available)
