# Queue-Keeper HTTP API Reference

Queue-Keeper exposes an HTTP API for webhook ingestion, health probing, event querying, observability, and administration. All endpoints listen on `host:8080` by default (configurable via `server.port` in `service.yaml`).

---

## Authentication

| Endpoint group | Auth required |
|----------------|---------------|
| `POST /webhook/{provider}` | No — signature validation is per-provider (see below) |
| `GET /health*`, `GET /ready` | No |
| `GET /api/*` | No |
| `GET /metrics`, `GET /debug/*` | No |
| `POST/PUT /admin/*`, `GET /admin/*` | **Yes** — Bearer token (see Admin Authentication) |

---

## Webhook Ingestion

### `POST /webhook/{provider}`

Accepts an incoming webhook payload from the named provider.

**URL Parameters**

| Parameter | Description |
|-----------|-------------|
| `provider` | URL-safe provider identifier (`[a-z0-9\-_]+`). Must match a registered provider ID. |

**Request Headers — GitHub provider**

| Header | Required | Description |
|--------|----------|-------------|
| `Content-Type` | Yes | Must be `application/json` |
| `X-GitHub-Event` | Yes | GitHub event type (e.g. `push`, `pull_request`) |
| `X-GitHub-Delivery` | Yes | GitHub delivery UUID |
| `X-Hub-Signature-256` | Conditional | HMAC-SHA256 signature. Required when `require_signature: true`. |

**Request Headers — Generic providers**

| Header | Required | Description |
|--------|----------|-------------|
| `Content-Type` | Yes | Must be `application/json` |
| Custom event type header | No | Reads the header named in `event_type_source.name` (e.g. `X-Atlassian-Event`) |
| Custom signature header | Conditional | Reads the header named in `signature.header_name` when signature validation is enabled |

**Request Body**

Raw JSON webhook payload (maximum 25 MB).

**Responses**

| Status | Description |
|--------|-------------|
| `200 OK` | Webhook processed successfully |
| `400 Bad Request` | Malformed request (missing required headers, invalid JSON, signature validation failed) |
| `404 Not Found` | Provider ID is not registered |
| `413 Payload Too Large` | Request body exceeds the 25 MB maximum |
| `429 Too Many Requests` | IP rate limit exceeded (10 authentication failures within 5 minutes) |
| `500 Internal Server Error` | Unexpected processing error |
| `503 Service Unavailable` | Transient processing failure; use `Retry-After` header |

**Response Body (200)**

```json
{
  "event_id": "01JQZM7XK4B3VYFNHD0G2T8P1X",
  "session_id": "myorg/myrepo/pull_request/42",
  "status": "processed",
  "message": "Webhook processed successfully"
}
```

**Response Body (400/404/413/500/503)**

```json
{
  "error": "Webhook provider not found: acme",
  "status": 404,
  "timestamp": "2026-04-08T10:00:00Z"
}
```

**curl Examples**

GitHub webhook:

```bash
curl -X POST https://queue-keeper.example.com/webhook/github \
  -H "Content-Type: application/json" \
  -H "X-GitHub-Event: push" \
  -H "X-GitHub-Delivery: 12345678-1234-1234-1234-123456789012" \
  -H "X-Hub-Signature-256: sha256=<hmac-value>" \
  -d '{"ref":"refs/heads/main","repository":{"full_name":"myorg/myrepo"}}'
```

Generic provider (Jira, direct mode):

```bash
curl -X POST https://queue-keeper.example.com/webhook/jira \
  -H "Content-Type: application/json" \
  -H "X-Atlassian-Event: jira:issue_created" \
  -H "X-Hub-Signature: sha256=<hmac-value>" \
  -d '{"webhookEvent":"jira:issue_created","issue":{"id":"10001","key":"PROJ-1"}}'
```

Generic provider (GitLab, wrap mode):

```bash
curl -X POST https://queue-keeper.example.com/webhook/gitlab \
  -H "Content-Type: application/json" \
  -H "X-Gitlab-Event: Merge Request Hook" \
  -H "X-Gitlab-Token: <shared-token>" \
  -d '{"project":{"path_with_namespace":"myorg/myrepo"},"object_attributes":{"iid":42,"action":"open"}}'
```

---

## Health Endpoints

### `GET /health`

Basic health check. Returns the aggregate service status without checking external dependencies.

**Responses**

| Status | Description |
|--------|-------------|
| `200 OK` | Service is healthy |
| `503 Service Unavailable` | Service is unhealthy |

**Response Body**

```json
{
  "status": "healthy",
  "version": "0.2.0",
  "timestamp": "2026-04-08T10:00:00Z",
  "checks": {
    "service": { "healthy": true, "message": "Service is running", "duration_ms": 0 },
    "providers": { "healthy": true, "message": "2 webhook provider(s) registered", "duration_ms": 0 }
  }
}
```

---

### `GET /health/deep`

Deep health check. Validates external dependencies (blob storage, queue, Key Vault).

**Responses**

| Status | Description |
|--------|-------------|
| `200 OK` | All dependencies are reachable |
| `503 Service Unavailable` | One or more dependencies are unavailable |

**Response Body**

```json
{
  "status": "healthy",
  "version": "0.2.0",
  "timestamp": "2026-04-08T10:00:00Z",
  "checks": {
    "service": { "healthy": true, "message": "Service is running", "duration_ms": 1 },
    "providers": { "healthy": true, "message": "2 webhook provider(s) registered", "duration_ms": 1 }
  }
}
```

---

### `GET /health/live`

Kubernetes liveness probe. Returns `200 OK` when the process is alive.

**Responses**

| Status | Description |
|--------|-------------|
| `200 OK` | Process is alive |

**Response Body**

```json
{
  "status": "alive",
  "version": "0.2.0",
  "timestamp": "2026-04-08T10:00:00Z",
  "checks": {}
}
```

---

### `GET /ready`

Kubernetes readiness probe. Returns `200 OK` when the service is ready to receive traffic (at least one provider is registered and all required dependencies are initialised). Returns `503 Service Unavailable` during startup or when critical dependencies are unavailable.

**Responses**

| Status | Description |
|--------|-------------|
| `200 OK` | Ready to accept traffic |
| `503 Service Unavailable` | Not yet ready |

**Response Body**

```json
{ "ready": true, "timestamp": "2026-04-08T10:00:00Z" }
```

---

## Event Query API

### `GET /api/events`

List stored webhook events. Results are paginated.

**Query Parameters**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `repository` | string | — | Filter by `owner/repo` |
| `event_type` | string | — | Filter by event type |
| `session_id` | string | — | Filter by session ID |
| `since` | ISO 8601 | — | Only events received after this timestamp |
| `page` | integer | 1 | Page number (1-based) |
| `per_page` | integer | 50 | Results per page (maximum 500) |

**Response Body (200)**

```json
{
  "events": [
    {
      "event_id": "01JQZM7XK4B3VYFNHD0G2T8P1X",
      "event_type": "pull_request",
      "repository": "myorg/myrepo",
      "session_id": "myorg/myrepo/pull_request/42",
      "occurred_at": "2026-04-08T10:00:00Z",
      "status": "processed"
    }
  ],
  "total": 1,
  "page": 1,
  "per_page": 50
}
```

---

### `GET /api/events/{event_id}`

Retrieve a specific event by its ULID.

**Path Parameters**

| Parameter | Description |
|-----------|-------------|
| `event_id` | ULID of the event (e.g. `01JQZM7XK4B3VYFNHD0G2T8P1X`) |

**Responses**

| Status | Description |
|--------|-------------|
| `200 OK` | Event found |
| `400 Bad Request` | `event_id` is not a valid ULID |
| `404 Not Found` | Event not found |

---

### `GET /api/sessions`

List active or historical sessions.

**Query Parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `repository` | string | Filter by `owner/repo` |
| `entity_type` | string | `pull_request`, `issue`, etc. |
| `status` | string | Filter by session status |
| `limit` | integer | Maximum number of results to return |

**Response Body (200)**

```json
{
  "sessions": [
    {
      "session_id": "myorg/myrepo/pull_request/42",
      "repository": "myorg/myrepo",
      "entity_type": "pull_request",
      "entity_id": "42",
      "status": "active",
      "event_count": 5,
      "last_activity": "2026-04-08T10:00:00Z"
    }
  ],
  "total": 1
}
```

---

### `GET /api/sessions/{session_id}`

Retrieve a specific session by ID.

**Path Parameters**

| Parameter | Description |
|-----------|-------------|
| `session_id` | Session ID in `owner/repo/entity_type/entity_id` format |

**Responses**

| Status | Description |
|--------|-------------|
| `200 OK` | Session found |
| `400 Bad Request` | `session_id` is not a valid session identifier |
| `404 Not Found` | Session not found |

**Response Body (200)**

```json
{
  "session": {
    "session_id": "myorg/myrepo/pull_request/42",
    "repository": {
      "id": 12345,
      "name": "myrepo",
      "full_name": "myorg/myrepo",
      "owner": { "id": 67890, "login": "myorg", "type": "Organization" },
      "private": false
    },
    "entity_type": "pull_request",
    "entity_id": "42",
    "status": "active",
    "created_at": "2026-04-08T09:00:00Z",
    "last_activity": "2026-04-08T10:00:00Z",
    "event_count": 5,
    "events": [
      {
        "event_id": "01JQZM7XK4B3VYFNHD0G2T8P1X",
        "event_type": "pull_request",
        "repository": "myorg/myrepo",
        "session_id": "myorg/myrepo/pull_request/42",
        "occurred_at": "2026-04-08T10:00:00Z",
        "status": "processed"
      }
    ]
  }
}
```

---

### `GET /api/stats`

Return aggregate statistics about processed events.

**Response Body (200)**

```json
{
  "total_events": 12500,
  "events_per_hour": 145.8,
  "active_sessions": 42,
  "error_rate": 0.0016,
  "uptime_seconds": 86400
}
```

---

## Observability Endpoints

### `GET /metrics`

Prometheus-format metrics. Suitable for scraping by Prometheus or Azure Monitor.

**Response**: Plain text in Prometheus exposition format.

```text
# HELP queue_keeper_webhooks_total Total webhooks received per provider
# TYPE queue_keeper_webhooks_total counter
queue_keeper_webhooks_total{provider="github",status="success"} 12000
queue_keeper_webhooks_total{provider="jira",status="success"} 500
```

---

### `GET /debug/pprof`

CPU profiling data. Registered unconditionally — restrict access at the network/gateway level in production.

---

### `GET /debug/vars`

Runtime variables dump. Registered unconditionally — restrict access at the network/gateway level in production.

---

## Admin API

All admin endpoints require a valid Bearer token presented in the
`Authorization: Bearer <token>` header. The token is configured via the
`security.admin_token` service configuration field or the `QK__SECURITY__ADMIN_TOKEN`
environment variable.

### `GET /admin/config`

Return the active service configuration. Note: webhook secrets are returned as
configured; redaction is not yet implemented. Production deployments should use
Key Vault references rather than literal secrets.

```bash
curl -H "Authorization: Bearer $ADMIN_TOKEN" \
  https://queue-keeper.example.com/admin/config
```

---

### `GET /admin/logging/level`

Return the current log level.

### `PUT /admin/logging/level`

Change the log level at runtime without restarting the service.

**Request Body**

```json
{ "level": "debug" }
```

Valid values: `trace`, `debug`, `info`, `warn`, `error`.

---

### `GET /admin/tracing/sampling`

Return the current OpenTelemetry trace sampling rate.

### `PUT /admin/tracing/sampling`

Change the trace sampling rate at runtime.

**Request Body**

```json
{ "sampling_ratio": 0.1 }
```

---

### `POST /admin/metrics/reset`

Reset all Prometheus counters and histograms to zero. Useful after a failed deployment.

---

### `POST /admin/events/{event_id}/replay`

Re-queue a previously stored event for reprocessing. The original payload is read
from blob storage and passed through the full routing pipeline.

> **Note:** This endpoint is not yet implemented and returns `501 Not Implemented`.

**Path Parameters**

| Parameter | Description |
|-----------|-------------|
| `event_id` | ULID of the event to replay |

**Responses**

| Status | Description |
|--------|-------------|
| `501 Not Implemented` | This endpoint is not yet implemented |

---

### `POST /admin/sessions/{session_id}/reset`

Reset the ordering session for the given session ID. Unblocks a stuck session when
a message in the session cannot be processed and must be skipped.

> **Note:** This endpoint is not yet implemented and returns `501 Not Implemented`.

**Path Parameters**

| Parameter | Description |
|-----------|-------------|
| `session_id` | Session ID in `owner/repo/entity_type/entity_id` format |

---

## Rate Limiting

Queue-Keeper enforces IP-based progressive rate limiting on authentication failures
(invalid or missing signatures):

| Failure count (5-minute window) | Response | Lock duration |
|---------------------------------|----------|---------------|
| < 10 | Request allowed | — |
| 10–50 | Rate restricted | 1 hour |
| > 50 | Complete block | 24 hours |

Rate limiting applies to all `/webhook/{provider}` and `/admin/*` endpoints.

---

## Error Response Format

Only the `/webhook/{provider}` endpoint returns a JSON body on error. All other
endpoints (`/api/*`, `/admin/*`) return bare HTTP status codes with no body.

Webhook error response structure:

```json
{
  "error": "Webhook provider not found: acme",
  "status": 404,
  "timestamp": "2026-04-08T10:00:00Z"
}
```

| Field | Description |
|-------|-------------|
| `error` | Human-readable error message |
| `status` | Numeric HTTP status code |
| `timestamp` | UTC timestamp of the error (RFC 3339) |
