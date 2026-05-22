# Handle Dead-Letter Messages

This guide shows how to inspect messages that have been moved to the dead-letter queue (DLQ) and how to reprocess or discard them. A message is dead-lettered after it has been delivered too many times without being successfully completed.

---

## Why messages are dead-lettered

A message moves to the dead-letter queue when:

- **Delivery count exceeded** — the message was delivered and abandoned (or the lock expired) more times than `max-delivery-count` (default: 10)
- **Message TTL expired** — the message was not consumed before the queue's `default-message-time-to-live` elapsed (default: 14 days)
- **Explicit dead-lettering** — your bot called `dead_letter_message()` to permanently reject a message

---

## Inspect dead-letter messages

The dead-letter queue for `queue-keeper-my-bot` is automatically created by Azure at `queue-keeper-my-bot/$DeadLetterQueue`.

**With Azure CLI:**

```bash
# Peek at dead-letter messages without consuming them
az servicebus queue show \
  --resource-group queue-keeper-rg \
  --namespace-name my-namespace \
  --name "queue-keeper-my-bot" \
  --query "countDetails.deadLetterMessageCount"
```

**With the Queue-Keeper CLI:**

```bash
queue-keeper events list --format table
```

Look for events with `status: dead_lettered`.

**With Python:**

```python
import json
import os

from azure.servicebus import ServiceBusClient

CONN_STR = os.environ["SERVICEBUS_CONNECTION_STRING"]
QUEUE_NAME = "queue-keeper-my-bot"

with ServiceBusClient.from_connection_string(CONN_STR) as client:
    with client.get_queue_receiver(
        QUEUE_NAME,
        sub_queue="deadletter",
        max_wait_time=5,
    ) as receiver:
        for msg in receiver:
            body = json.loads(str(msg))
            reason = msg.dead_letter_reason
            description = msg.dead_letter_error_description
            print(f"Event ID: {body.get('event_id')}")
            print(f"Reason: {reason} — {description}")
            print()
            receiver.abandon_message(msg)  # Leave in DLQ for now
```

---

## Reprocess a dead-letter message

To reprocess, move the message back to the main queue. The simplest way is to use the Queue-Keeper replay feature, which re-routes the event from Blob Storage rather than touching the DLQ directly:

```bash
queue-keeper events replay <event_id> --force
```

Alternatively, read the dead-letter message and re-enqueue it:

```python
with ServiceBusClient.from_connection_string(CONN_STR) as client:
    sender = client.get_queue_sender(QUEUE_NAME)
    with client.get_queue_receiver(
        QUEUE_NAME,
        sub_queue="deadletter",
        max_wait_time=5,
    ) as receiver:
        for msg in receiver:
            body = json.loads(str(msg))
            # Re-send to main queue
            from azure.servicebus import ServiceBusMessage
            new_msg = ServiceBusMessage(
                str(msg),
                session_id=msg.session_id,  # Preserve session for ordered bots
                application_properties=msg.application_properties,
            )
            sender.send_messages(new_msg)
            receiver.complete_message(msg)  # Remove from DLQ
```

---

## Discard dead-letter messages

If a dead-lettered message is invalid and should not be reprocessed, complete it from the DLQ to remove it permanently:

```python
with client.get_queue_receiver(
    QUEUE_NAME,
    sub_queue="deadletter",
    max_wait_time=5,
) as receiver:
    for msg in receiver:
        body = json.loads(str(msg))
        event_id = body.get("event_id", "unknown")
        print(f"Discarding dead-letter event {event_id}")
        receiver.complete_message(msg)
```

---

## Preventing dead-lettering

To reduce dead-letter accumulation:

- **Make processing idempotent** — safe to retry automatically. Use `event_id` to skip work already done
- **Distinguish transient vs permanent errors** — abandon transient failures (network errors) so Azure retries them; dead-letter permanent failures (schema validation errors) immediately with `receiver.dead_letter_message(msg, reason="invalid schema")`
- **Extend the lock** for slow processing operations so the lock does not expire mid-processing
- **Increase `max-delivery-count`** if 10 retries is not enough for your workload

See [Deduplicate Replayed Events](deduplicate-events.md) for implementing idempotent processing.
