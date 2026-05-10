# Monitor the Service

This guide covers the available monitoring surfaces in Queue-Keeper: health endpoints, the Prometheus-compatible metrics endpoint, structured logs, and recommended alert conditions.

---

## Health endpoints

Queue-Keeper exposes two health endpoints. Both return JSON and require no authentication.

### `GET /health` — liveness

Returns aggregate health without checking external dependencies. Use for container liveness probes and load balancer health checks.

```bash
curl -s http://localhost:8080/health | python3 -m json.tool
```

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

### `GET /ready` — readiness

Checks external dependencies (queue, key vault, blob storage). Use for Kubernetes readiness probes, so traffic is not sent until all dependencies are verified.

```bash
curl -s http://localhost:8080/ready
```

Returns `200 OK` when ready, `503 Service Unavailable` when not yet ready.

---

## Metrics endpoint

### `GET /metrics`

Exposes Prometheus-format metrics. Scrape this endpoint with your metrics collector.

```bash
curl -s http://localhost:8080/metrics | head -40
```

### Key metrics

**Webhook processing:**

| Metric | Type | Description |
|---|---|---|
| `webhook_requests_total` | Counter | Total webhook requests, labelled by `provider` and `status` |
| `webhook_duration_seconds` | Histogram | End-to-end processing latency |
| `webhook_validation_failures_total` | Counter | Requests rejected due to invalid signature or payload |
| `webhook_payload_size_bytes` | Histogram | Incoming webhook payload size distribution |

**Queue routing:**

| Metric | Type | Description |
|---|---|---|
| `queue_messages_sent_total` | Counter | Messages sent to bot queues, labelled by `queue` |
| `queue_routing_duration_seconds` | Histogram | Time to route an event to all matching queues |
| `dead_letter_messages_total` | Counter | Messages that exhausted retries and were dead-lettered |

**Circuit breakers:**

| Metric | Type | Description |
|---|---|---|
| `circuit_breaker_state` | Gauge | Current state per dependency (0=closed, 1=open, 2=half-open) |
| `circuit_breaker_trips_total` | Counter | Number of times the circuit breaker opened |

### Prometheus scrape config

```yaml
scrape_configs:
  - job_name: queue-keeper
    static_configs:
      - targets: ["queue-keeper:8080"]
    metrics_path: /metrics
```

---

## Structured logs

Queue-Keeper emits JSON logs by default (set `logging.format: "text"` for human-readable output). Every log entry includes:

- `timestamp` — ISO 8601 UTC
- `level` — trace, debug, info, warn, error
- `correlation_id` — links the log to a specific webhook delivery (also `delivery_id` for GitHub)
- `event_id` — when the event has been assigned an ID
- `repository` — source repository when processing a GitHub event

**Filter logs by correlation ID** (useful when investigating a specific GitHub delivery):

```bash
# Docker
docker logs queue-keeper 2>&1 | grep '"correlation_id":"00-4bf92f3577b34da6a"'

# Kubernetes
kubectl -n automation logs -l app=queue-keeper \
  | grep '"correlation_id":"00-4bf92f3577b34da6a"'
```

---

## Recommended alerts

Configure these alerts in your metrics platform (Grafana, Azure Monitor, etc.):

| Alert | Condition | Severity | Action |
|---|---|---|---|
| High error rate | `rate(webhook_requests_total{status=~"5.."}[5m]) / rate(webhook_requests_total[5m]) > 0.01` | High | Investigate logs, check circuit breaker state |
| Slow webhook processing | `histogram_quantile(0.95, webhook_duration_seconds) > 0.8` | Warning | Check queue latency, scale if needed |
| Circuit breaker open | `circuit_breaker_state > 0` | High | Check service bus / key vault connectivity |
| Dead letter queue growing | `increase(dead_letter_messages_total[10m]) > 10` | Warning | Inspect dead letters, check bot processing |
| Service unhealthy | `/health` returns non-200 | Critical | Immediate investigation |

---

## Debug endpoints

The following endpoints are available without authentication in development builds. In production, restrict them at the network boundary.

| Endpoint | Description |
|---|---|
| `GET /debug/pprof` | Performance profiling data |
| `GET /debug/vars` | Internal counters and runtime variables |
| `PUT /admin/log-level` | Dynamically adjust log level without restart |

### Change log level at runtime

```bash
curl -s -X PUT http://localhost:8080/admin/log-level \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"level": "debug"}'
```

Remember to revert to `info` after debugging to avoid excessive log volume.
