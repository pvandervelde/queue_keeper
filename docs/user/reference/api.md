# HTTP API Reference

Queue-Keeper exposes an HTTP API on port 8080 (configurable). All endpoints expect and return `application/json` unless noted otherwise.

---

## Authentication

| Endpoint group | Auth required |
|---|---|
| `POST /webhook/{provider}` | No — validated per-provider via HMAC signature |
| `GET /health`, `GET /ready` | No |
| `GET /metrics` | No |
| `GET /api/*` | No |
| `GET /debug/*` | No (restrict at network boundary in production) |
| `POST/PUT/GET /admin/*` | Yes — Bearer token |

Admin authentication:

```
Authorization: Bearer <admin-token>
```

---

## Webhook Ingestion

### `POST /webhook/{provider}`

Accepts an incoming webhook payload from the named provider.

**Path parameters**

| Parameter | Description |
|---|---|
| `provider` | Provider identifier (`[a-z0-9\-_]+`). Must match a registered provider. |

**Request headers — GitHub**

| Header | Required | Description |
|---|---|---|
| `Content-Type` | Yes | `application/json` |
| `X-GitHub-Event` | Yes | GitHub event type (e.g. `push`, `pull_request`) |
| `X-GitHub-Delivery` | Yes | GitHub delivery UUID |
| `X-Hub-Signature-256` | Conditional | HMAC-SHA256 signature. Required when `require_signature: true`. |

**Request headers — Generic providers**

| Header | Required | Description |
|---|---|---|
| `Content-Type` | Yes | `application/json` |
| Event type header | No | Header name from `event_type_source.name` |
| Signature header | Conditional | Header name from `signature.header_name` |

**Request body**

Raw JSON webhook payload. Maximum `server.max_body_size` bytes (default: 10 MB). Configure a higher limit via `server.max_body_size` in `service.yaml`.

**Responses**

| Status | Description |
|---|---|
| `200 OK` | Processed successfully |
| `400 Bad Request` | Missing headers, invalid JSON, or signature mismatch |
| `404 Not Found` | Provider ID not registered |
| `413 Payload Too Large` | Body exceeds `server.max_body_size` (default 10 MB) |
| `429 Too Many Requests` | IP rate limit exceeded |
| `500 Internal Server Error` | Unexpected error |
| `503 Service Unavailable` | Transient failure; retry after `Retry-After` seconds |

**Response body (200)**

```json
{
  "event_id": "01JQZM7XK4B3VYFNHD0G2T8P1X",
  "session_id": "myorg/myrepo/pull_request/42",
  "status": "processed",
  "message": "Webhook processed successfully"
}
```

**Response body (error)**

```json
{
  "error": "Webhook provider not found: acme",
  "status": 404,
  "timestamp": "2026-05-07T10:00:00Z"
}
```

---

## Trace Context

Queue-Keeper checks these headers for an upstream trace ID (first non-empty value wins):

| Priority | Header | Standard |
|---|---|---|
| 1 | `traceparent` | W3C Trace Context |
| 2 | `X-Correlation-ID` | Queue-Keeper convention |
| 3 | `X-Request-ID` | Common de-facto standard |

The extracted value becomes `correlation_id` on every message placed on the queue. Absent all headers, a UUID v4 is generated.

---

## Health Endpoints

### `GET /health`

Liveness check. Does not check external dependencies.

**Response (200)**

```json
{
  "status": "healthy",
  "version": "0.2.1",
  "timestamp": "2026-05-07T10:00:00Z",
  "checks": {
    "service":   { "healthy": true, "message": "Service is running", "duration_ms": 0 },
    "providers": { "healthy": true, "message": "2 webhook provider(s) registered", "duration_ms": 0 }
  }
}
```

### `GET /ready`

Readiness check. Verifies external dependencies are reachable.

Returns `200 OK` when ready, `503` when not.

---

## Metrics

### `GET /metrics`

Prometheus-format metrics. See [Monitor the Service](../how-to/operators/monitor.md) for the full metric list.

---

## Event Query API

These endpoints require no authentication and return information about processed events.

### `GET /api/events`

Returns a paginated list of recently processed events.

**Query parameters**

| Parameter | Type | Default | Description |
|---|---|---|---|
| `limit` | integer | `50` | Max events to return (1–500) |
| `event_type` | string | — | Filter by event type |
| `repository` | string | — | Filter by `owner/repo` |
| `session` | string | — | Filter by session ID |
| `since` | ISO 8601 | — | Events after this timestamp |

**Response (200)**

```json
{
  "events": [
    {
      "event_id": "01JQZM7XK4B3VYFNHD0G2T8P1X",
      "provider": "github",
      "event_type": "pull_request",
      "action": "opened",
      "session_id": "myorg/myrepo/pull_request/42",
      "correlation_id": "00-4bf92f...",
      "received_at": "2026-05-07T10:00:00.000Z",
      "status": "processed"
    }
  ],
  "total": 1,
  "limit": 50
}
```

### `GET /api/events/{event_id}`

Returns full details for a specific event.

**Response (200)**

```json
{
  "event_id": "01JQZM7XK4B3VYFNHD0G2T8P1X",
  "provider": "github",
  "event_type": "pull_request",
  "action": "opened",
  "session_id": "myorg/myrepo/pull_request/42",
  "correlation_id": "00-4bf92f...",
  "received_at": "2026-05-07T10:00:00.000Z",
  "processed_at": "2026-05-07T10:00:00.123Z",
  "status": "processed",
  "payload": { "...": "original webhook body" }
}
```

---

## Admin API

All admin endpoints require `Authorization: Bearer <token>`.

### `POST /admin/events/{event_id}/replay`

Replays an event from Blob Storage.

**Request body**

```json
{
  "target_queue": "queue-keeper-my-bot"   // optional — defaults to all matching queues
}
```

**Response (200)**

```json
{
  "event_id": "01JQZM7XK4B3VYFNHD0G2T8P1X",
  "status": "replayed"
}
```

### `PUT /admin/log-level`

Dynamically changes the log level without restarting the service.

**Request body**

```json
{ "level": "debug" }
```

**Response (200)**

```json
{ "previous_level": "info", "current_level": "debug" }
```

---

## Debug Endpoints

These endpoints are intended for development and troubleshooting. Restrict them at the network boundary in production.

| Endpoint | Description |
|---|---|
| `GET /debug/pprof` | Performance profiling data |
| `GET /debug/vars` | Internal counters and runtime state |
