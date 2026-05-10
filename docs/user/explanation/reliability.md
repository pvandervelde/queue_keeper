# Reliability

Queue-Keeper is designed to deliver every webhook reliably, even when downstream services (Azure Service Bus, Blob Storage, Key Vault) experience transient failures. This page explains the reliability mechanisms and the design trade-offs behind them.

---

## The reliability goal

GitHub expects a response within 10 seconds and will retry delivery if it does not receive one. Queue-Keeper's target is **< 1 second**. If Queue-Keeper itself is unavailable, GitHub retries for up to 3 days.

The reliability goal for downstream bot delivery is **at-least-once**: every event that Queue-Keeper successfully acknowledges to GitHub will eventually appear on the bot's queue, even if the first delivery attempt to Service Bus fails.

---

## Retry policy

When a transient error occurs (network timeout, Azure Service Bus throttling, temporary unavailability), Queue-Keeper retries using **exponential backoff with jitter**:

| Attempt | Delay range |
|---|---|
| 1st retry | 75–125 ms |
| 2nd retry | 150–250 ms |
| 3rd retry | 300–500 ms |
| 4th retry | 600–1000 ms |
| 5th retry | 1200–2000 ms |

Jitter (±25%) prevents thundering-herd problems when multiple in-flight requests all retry at the same moment.

**Permanent errors** (invalid credentials, queue not found, malformed payload) are not retried — they fail immediately with an appropriate HTTP error code.

**Maximum retry window**: roughly 3–4 seconds total, which keeps the response to GitHub within the 10-second hard limit even under sustained transient failures.

---

## Circuit breaker

Queue-Keeper wraps its connections to the message queue, object storage, and secret store with circuit breakers. This prevents a failing downstream service from monopolising all request processing capacity.

### States

```
         failure threshold reached
Closed ─────────────────────────── Open
  ▲                                  │
  │ success threshold reached   after timeout
  │                                  │
  └──────────── Half-Open ◀──────────┘
               (probe requests)
```

| State | Behaviour |
|---|---|
| **Closed** | Normal operation; failures are counted |
| **Open** | Requests fast-fail immediately; no downstream calls |
| **Half-Open** | A limited number of probe requests are allowed through |

### Thresholds by service

| Service | Open after | Recover after | Reset period |
|---|---|---|---|
| Message Queue | 5 failures | 3 successes | 30 s |
| Object Storage | 3 failures | 2 successes | 10 s |
| Secret Store | 3 failures | 2 successes | 15 s |

### Graceful degradation

**Object Storage failure**: If object storage is unavailable, Queue-Keeper continues processing and routing events to the message queue. The audit record is lost for events processed during the outage, but bot delivery is unaffected. This is a deliberate trade-off: audit records are valuable, but not worth blocking bot delivery.

**Secret Store failure**: Webhook signature validation uses cached secrets. If the secret store is unavailable and the cache has not expired, processing continues normally. If the cache expires while the secret store is down, Queue-Keeper will reject incoming webhooks (cannot validate signatures without the secret) until the secret store recovers.

**Message Queue failure**: If the message queue is unavailable, Queue-Keeper cannot deliver events to bot queues. The circuit breaker opens, and subsequent webhooks receive `503 Service Unavailable` with a `Retry-After` header. GitHub will retry delivery when the service recovers.

---

## Dead-letter queue

Most queue backends supported by `queue-runtime` provide a dead-letter queue (DLQ) associated with each regular queue. A message moves to the DLQ when:

- It is delivered more times than `max-delivery-count` (default: 10) without being completed
- It exceeds the queue's message time-to-live (default: 14 days) without being consumed
- The consuming bot explicitly requests dead-lettering

The DLQ preserves the original message, the failure reason, and a description. Operators can inspect the DLQ to diagnose persistent failures and replay events via the CLI or HTTP API.

See [Handle Dead-Letter Messages](../how-to/bot-developers/handle-dead-letters.md) for operational guidance.

---

## Object storage audit trail and replay

Every webhook received by Queue-Keeper is written to object storage at:

```
{year}/{month}/{day}/{event_id}.json
```

This serves two purposes:

1. **Audit**: An immutable record of exactly what was received, when, and from which provider
2. **Replay**: Events can be reprocessed from the stored payload at any future time, using either the CLI or the HTTP API

Replay is idempotent — the `event_id` is stable and derived from the original payload, so your bot can detect and skip duplicate deliveries using `event_id`. See [Deduplicate Replayed Events](../how-to/bot-developers/deduplicate-events.md).

---

## At-least-once vs exactly-once

Queue-Keeper provides **at-least-once delivery**. In practice, most events are delivered exactly once, but the following scenarios can cause duplicates:

- Queue-Keeper retried a Service Bus send that actually succeeded (the first attempt timed out after the message was enqueued)
- An operator replayed an event
- Azure Service Bus re-delivered a message whose lock expired before the consumer completed it

Your bot must handle duplicates safely. The `event_id` field is the deduplication key. See [Deduplicate Replayed Events](../how-to/bot-developers/deduplicate-events.md) for implementation patterns.

**Exactly-once delivery is not provided.** Achieving it would require distributed transactions spanning multiple services, which would significantly increase latency and reduce availability.
