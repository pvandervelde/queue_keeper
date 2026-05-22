# Deduplicate Replayed Events

This guide shows how to use the `event_id` field in a `WrappedEvent` to detect and safely skip events your bot has already processed. This protects against duplicate processing during Queue-Keeper replays, message redeliveries after lock/visibility expiry, and other retry scenarios.

---

## Why duplicates happen

Events can arrive more than once because:

- **Lock expiry** — your bot held the message lock too long; Azure re-delivers to another consumer
- **Abandon on transient error** — your bot abandoned the message to retry; the same message is redelivered
- **Replay** — an operator used `queue-keeper events replay` to reprocess an event, possibly sending it again to all matching bots
- **Queue consumer restart** — a message was received but the in-progress confirmation was lost before the bot completed the message

---

## The `event_id` field

Every `WrappedEvent` carries a globally unique `event_id`:

```json
{
  "event_id": "01JQZM7XK4B3VYFNHD0G2T8P1X",
  ...
}
```

The ID is a [ULID](https://github.com/ulid/spec) — a 26-character, lexicographically sortable, globally unique string. Queue-Keeper generates the same `event_id` for the same original webhook payload each time it processes that webhook, so replays and retries produce the same ID.

---

## Implementation pattern

The standard pattern is a processed-event store. Before processing an event, check whether you have already handled that `event_id`. If yes, complete the message and skip.

=== "Python (in-memory, dev only)"

    ```python
    # Simple set — lost on restart, only suitable for development
    processed_ids: set[str] = set()

    def is_duplicate(event_id: str) -> bool:
        return event_id in processed_ids

    def mark_processed(event_id: str) -> None:
        processed_ids.add(event_id)
    ```

=== "Python (Redis)"

    ```python
    import redis

    r = redis.Redis.from_url(os.environ["REDIS_URL"])
    TTL_SECONDS = 7 * 24 * 3600  # 7 days, matching queue message TTL

    def is_duplicate(event_id: str) -> bool:
        return r.exists(f"processed:{event_id}") == 1

    def mark_processed(event_id: str) -> None:
        r.set(f"processed:{event_id}", "1", ex=TTL_SECONDS)
    ```

=== "Python (PostgreSQL)"

    ```python
    import psycopg2

    conn = psycopg2.connect(os.environ["DATABASE_URL"])

    def is_duplicate(event_id: str) -> bool:
        with conn.cursor() as cur:
            cur.execute(
                "SELECT 1 FROM processed_events WHERE event_id = %s", (event_id,)
            )
            return cur.fetchone() is not None

    def mark_processed(event_id: str) -> None:
        with conn.cursor() as cur:
            cur.execute(
                "INSERT INTO processed_events (event_id, processed_at) VALUES (%s, NOW())"
                " ON CONFLICT DO NOTHING",
                (event_id,),
            )
        conn.commit()
    ```

---

## Full receiver example

```python
def process_message(receiver, msg) -> None:
    event = json.loads(str(msg))
    event_id = event["event_id"]

    if is_duplicate(event_id):
        logger.info("Skipping duplicate event %s", event_id)
        receiver.complete_message(msg)
        return

    try:
        do_work(event)
        mark_processed(event_id)
        receiver.complete_message(msg)
    except TransientError as exc:
        logger.warning("Transient error on %s, will retry: %s", event_id, exc)
        receiver.abandon_message(msg)
    except PermanentError as exc:
        logger.error("Permanent error on %s, dead-lettering: %s", event_id, exc)
        receiver.dead_letter_message(msg, reason=str(exc))
```

---

## Choosing a store

| Store | Suitable for | Notes |
|---|---|---|
| In-memory set | Local development only | Lost on restart |
| Redis | Stateless bots with high throughput | TTL-based expiry, fast |
| PostgreSQL / other DB | Bots that already use a database | Transactions possible; mark processed atomically with business logic |
| Azure Table Storage | Azure-native, low cost | Simple key lookup |

---

## Retention window

Keep processed event IDs for at least as long as your queue's `default-message-time-to-live`. Once a message expires from the queue it cannot be redelivered, so the deduplication record can be safely deleted.

Default queue TTL in these guides: 14 days. Set your store's TTL to at least 14 days (1,209,600 seconds).
