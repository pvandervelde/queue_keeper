# Replay Events

This guide shows how to reprocess a webhook event that was persisted to Blob Storage. Replay is useful when a bot was misconfigured, a downstream queue was unavailable, or you need to re-run processing after a bug fix.

## Prerequisites

- Queue-Keeper service running and accessible
- The event ID you want to replay (from logs, the `events list` CLI command, or the API)
- Sufficient permissions: admin token for the HTTP API, or direct CLI access

---

## Find the event ID

**From the CLI:**

```bash
# List recent events
queue-keeper events list --limit 20 --format table

# Filter by event type
queue-keeper events list --event-type pull_request --limit 20

# Filter by repository
queue-keeper events list --repository myorg/myrepo --limit 20
```

**From the API:**

```bash
curl -s "http://localhost:8080/api/events?limit=20&event_type=push" \
  -H "Authorization: Bearer $ADMIN_TOKEN" | python3 -m json.tool
```

The event ID is the `event_id` field in the response, for example `01JQZM7XK4B3VYFNHD0G2T8P1X`.

---

## Inspect the event before replaying

Verify you have the correct event before replaying:

```bash
queue-keeper events show 01JQZM7XK4B3VYFNHD0G2T8P1X --format yaml
```

Add `--raw` to see the original webhook payload as it was received from GitHub.

---

## Replay using the CLI

```bash
queue-keeper events replay 01JQZM7XK4B3VYFNHD0G2T8P1X
```

By default the event is routed to all bot queues that currently match its event type — the same routing rules applied when the event was first processed. To target a specific queue:

```bash
queue-keeper events replay 01JQZM7XK4B3VYFNHD0G2T8P1X \
  --queue queue-keeper-task-tactician
```

Use `--force` to replay an event even if it has already been successfully processed:

```bash
queue-keeper events replay 01JQZM7XK4B3VYFNHD0G2T8P1X --force
```

---

## Replay using the HTTP API

```bash
curl -s -X POST "http://localhost:8080/admin/events/01JQZM7XK4B3VYFNHD0G2T8P1X/replay" \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{}'
```

To target a specific queue:

```bash
curl -s -X POST "http://localhost:8080/admin/events/01JQZM7XK4B3VYFNHD0G2T8P1X/replay" \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"target_queue": "queue-keeper-task-tactician"}'
```

---

## Idempotency

Replay is safe to run multiple times. Queue-Keeper generates the same `event_id` from the original payload — your bot should use `event_id` to detect and skip duplicate deliveries. See [Deduplicate Replayed Events](../../how-to/bot-developers/deduplicate-events.md) for implementation guidance.

---

## Bulk replay

To replay a range of events (e.g. after a queue outage), pipe event IDs from `events list`:

```bash
queue-keeper events list \
  --since "2026-05-01T00:00:00Z" \
  --event-type push \
  --format json \
  | python3 -c "
import json, sys, subprocess
for ev in json.load(sys.stdin)['events']:
    subprocess.run(['queue-keeper', 'events', 'replay', ev['event_id']], check=True)
"
```

!!! tip
    For large bulk replays, replay to a single queue at a time and confirm the bot processes events correctly before continuing to the next batch.
