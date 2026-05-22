# Configuration Reference

Queue-Keeper is configured through two YAML files:

- **`service.yaml`** — HTTP server, providers, queue backend, security, and logging settings
- **`bot-config.yaml`** — bot subscriptions and event routing rules

Both files are loaded at startup and are immutable at runtime. A service restart is required after any change.

The path to `service.yaml` is supplied via the `--config` flag or the `QUEUE_KEEPER_CONFIG` environment variable. The bot configuration file path is specified within `service.yaml`.

---

## `service.yaml`

### Top-level structure

```yaml
server:    { ... }           # HTTP server settings
webhooks:  { ... }           # Webhook processing settings
security:  { ... }           # Rate limiting and authentication
logging:   { ... }           # Log level and format
providers: [ ... ]           # GitHub-style built-in providers
generic_providers: [ ... ]   # Configuration-driven generic providers
key_vault: { ... }           # Azure Key Vault connection
queue:     { ... }           # Queue backend selection
```

---

### `server`

| Field | Type | Default | Description |
|---|---|---|---|
| `port` | integer | `8080` | TCP port to listen on |
| `host` | string | `"0.0.0.0"` | Interface to bind to |
| `timeout_seconds` | integer | `30` | Request timeout in seconds |
| `shutdown_timeout_seconds` | integer | `30` | Graceful shutdown timeout in seconds |
| `max_body_size` | integer | `10485760` (10 MB) | Maximum request body size in bytes |
| `enable_cors` | boolean | `true` | Enable CORS headers |
| `enable_compression` | boolean | `true` | Enable response compression |

```yaml
server:
  port: 8080
  host: "0.0.0.0"
  timeout_seconds: 30
  shutdown_timeout_seconds: 30
  max_body_size: 10485760
  enable_cors: true
  enable_compression: true
```

---

### `webhooks`

| Field | Type | Default | Description |
|---|---|---|---|
| `endpoint_path` | string | `"/webhook"` | Base URL path for webhook endpoints |
| `require_signature` | boolean | `true` | Global default: reject requests without a valid HMAC signature (overridden per-provider by `providers[*].require_signature`) |
| `store_payloads` | boolean | `true` | Write raw payloads to object storage for audit and replay |
| `allowed_event_types` | list | `[]` (all) | Global event-type allowlist; empty list accepts all types |
| `rate_limit_per_repo` | integer or null | `100` | Max events per repository per minute; `null` disables the limit |

```yaml
webhooks:
  endpoint_path: "/webhook"
  require_signature: true
  store_payloads: true
  allowed_event_types: []
  rate_limit_per_repo: 100
```

---

### `security`

| Field | Type | Default | Description |
|---|---|---|---|
| `enable_rate_limiting` | boolean | `true` | Enable global request rate limiting |
| `global_rate_limit` | integer | `1000` | Max requests per minute (service-wide) |
| `enable_ip_rate_limiting` | boolean | `true` | Enable per-IP rate limiting |
| `ip_rate_limit` | integer | `100` | Max requests per minute per source IP |
| `log_requests` | boolean | `true` | Log each incoming request; set to `false` to reduce log volume |
| `auth_failure_threshold` | integer | `10` | Auth failures before an IP enters the rate-restricted tier |
| `auth_block_threshold` | integer | `50` | Auth failures before an IP is fully blocked |
| `auth_failure_window_secs` | integer | `300` | Sliding window for auth failure counting (seconds) |
| `auth_rate_restrict_duration_secs` | integer | `3600` | Duration an IP stays in rate-restricted tier (seconds) |
| `auth_block_duration_secs` | integer | `86400` | Duration an IP stays fully blocked (seconds) |
| `admin_api_key` | string | none | Bearer token required for `/admin/**` endpoints. Set via `QK__SECURITY__ADMIN_API_KEY`; do not commit to source control |

```yaml
security:
  enable_rate_limiting: true
  global_rate_limit: 1000
  enable_ip_rate_limiting: true
  ip_rate_limit: 100
  log_requests: true
  auth_failure_threshold: 10
  auth_block_threshold: 50
  auth_failure_window_secs: 300
  auth_rate_restrict_duration_secs: 3600
  auth_block_duration_secs: 86400
```

!!! warning "Admin API key"
    Never store `admin_api_key` in a committed YAML file. Inject it at runtime via `QK__SECURITY__ADMIN_API_KEY`.

---

### `logging`

| Field | Type | Default | Description |
|---|---|---|---|
| `level` | string | `"info"` | Log level: `trace`, `debug`, `info`, `warn`, `error` |
| `json_format` | boolean | `false` | `true` emits structured JSON logs; `false` emits human-readable text |
| `file_path` | string | none | Optional path to write logs to a file in addition to stdout |

```yaml
logging:
  level: "info"
  json_format: true
```

---

### `providers`

An array of built-in provider entries. Each entry enables one webhook endpoint at `/webhook/{id}`.

| Field | Type | Required | Description |
|---|---|---|---|
| `id` | string | yes | URL-safe provider identifier (`[a-z0-9\-_]+`) |
| `require_signature` | boolean | yes | Reject requests without a valid HMAC signature |
| `secret` | object | when `require_signature: true` | Secret source (see below) |

**Secret source — Key Vault:**

```yaml
secret:
  type: key_vault
  secret_name: "github-webhook-secret"
```

**Secret source — literal (development only):**

```yaml
secret:
  type: literal
  value: "my-dev-secret"
```

---

### `generic_providers`

An array of configuration-driven provider entries. Each enables one webhook endpoint at `/webhook/{provider_id}`.

| Field | Type | Required | Description |
|---|---|---|---|
| `provider_id` | string | yes | URL-safe provider identifier |
| `processing_mode` | string | yes | `wrap` or `direct` |
| `target_queue` | string | when `direct` | Queue name for direct-mode delivery |
| `event_type_source` | object | yes | Where to read the event type from |
| `signature` | object | no | Signature validation config |

**`event_type_source` variants:**

```yaml
# From a header
event_type_source:
  type: header
  name: "X-Atlassian-Event"

# From a JSON body field
event_type_source:
  type: body_field
  field_path: "type"
```

**`signature` block:**

```yaml
signature:
  header_name: "X-Hub-Signature"
  algorithm: sha256          # sha256 | sha1 | plain
  secret:
    type: key_vault
    secret_name: "my-provider-secret"
```

---

### `key_vault`

Required when any provider uses `type: key_vault`. Configures the Azure Key Vault instance from which Queue-Keeper fetches webhook secrets.

| Field | Type | Required | Description |
|---|---|---|---|
| `vault_url` | string | yes | Azure Key Vault URL, e.g. `https://my-vault.vault.azure.net` |

```yaml
key_vault:
  vault_url: "https://my-vault.vault.azure.net"
```

On AWS or other platforms, use `type: environment_variable` for the provider secret instead:

```yaml
providers:
  - id: "github"
    require_signature: true
    secret:
      type: environment_variable
      env_var_name: "GITHUB_WEBHOOK_SECRET"
```

The environment variable is read once at startup and treated as a literal thereafter.

---

### `queue`

Selects and configures the queue backend. Exactly one variant must be specified.

!!! note "Supported queue backends"
    Queue-Keeper currently supports **Azure Service Bus** and **AWS SQS** as production queue backends. RabbitMQ and NATS are not currently supported.

**In-memory (development only):**

```yaml
queue:
  provider: in_memory
```

Events are not persisted across restarts.

**Azure Service Bus — managed identity (production):**

```yaml
queue:
  provider: azure_service_bus
  namespace: my-namespace.servicebus.windows.net
```

**Azure Service Bus — connection string (dev/test):**

```yaml
queue:
  provider: azure_service_bus
  connection_string: "Endpoint=sb://..."
```

**AWS SQS — IAM role (production):**

```yaml
queue:
  provider: aws_sqs
  region: us-east-1
```

The AWS SDK credential chain is used (ECS task role, EC2 instance profile, environment variables). Do not embed credentials in `service.yaml`.

---

## `bot-config.yaml`

### Top-level structure

```yaml
bots:
  - name: string              # Unique bot identifier
    queue: string             # Target queue name
    events: [string]          # Event patterns to subscribe to
    ordered: boolean          # Session-based FIFO ordering
    repository_filter: ...    # Optional — filter by repository
    config: ...               # Optional — bot-specific settings
```

---

### `name`

Unique identifier for the bot.

- Required
- 1–50 characters
- Allowed: letters, numbers, hyphens
- Must not start or end with a hyphen
- Example: `"task-tactician"`, `"merge-warden"`

---

### `queue`

Target queue name for this bot's messages. With Azure Service Bus this is the queue name; with AWS SQS this is the queue name or ARN.

- Required
- Must start with `queue-keeper-`
- 1–260 characters; allowed characters: letters, numbers, `.`, `-`, `_`
- Example: `"queue-keeper-task-tactician"`

---

### `events`

Array of event patterns this bot receives. At least one pattern is required.

| Pattern | Matches |
|---|---|
| `"issues.opened"` | Exact event and action |
| `"issues.*"` | All issue actions |
| `"*"` | All events from all providers |
| `"!issues.deleted"` | Exclusion — use alongside inclusion patterns |

Exclusions are processed after inclusions. Example:

```yaml
events:
  - "issues.*"
  - "!issues.deleted"
```

---

### `ordered`

Boolean. Controls whether session-based FIFO ordering is applied.

- `true` — sets `SessionId` on each outgoing message; the target queue must have sessions enabled
- `false` — no session ID; standard parallel delivery

---

### `repository_filter`

Optional. Restricts delivery to events from specific repositories. Uses YAML tag syntax.

```yaml
# Single exact repository
repository_filter:
  !exact
  owner: "myorg"
  name: "myrepo"

# Multiple repositories (OR)
repository_filter:
  !any_of
  - !exact
    owner: "myorg"
    name: "repo1"
  - !exact
    owner: "myorg"
    name: "repo2"

# All repositories in an org
repository_filter: !owner myorg

# Name pattern (regex)
repository_filter: !name_pattern ^prod-.*

# AND combination
repository_filter:
  !all_of
  - !owner myorg
  - !name_pattern .*-service$
```

---

### `config`

Optional bot-specific settings passed in the `WrappedEvent` envelope:

```yaml
config:
  settings:
    priority: "high"
    timeout_seconds: 300
    feature_flag: true
```

These values are available in your bot via `event["payload"]["bot_config"]` (exact path depends on implementation).
