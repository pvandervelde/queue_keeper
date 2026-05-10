# Use Ordered Delivery

This guide configures ordered (FIFO) delivery so your bot receives events for the same pull request or issue in the order they arrived. It covers queue creation, bot configuration, and writing a session-aware receiver.

For background on why sessions are needed, see [Ordering and Sessions](../../explanation/ordering-sessions.md).

---

## When to use ordered delivery

Set `ordered: true` when your bot:

- Maintains per-entity state (e.g. tracks issue status over its lifecycle)
- Would produce wrong results if events arrive out of order (e.g. processes `closed` before `opened`)
- Needs to prevent concurrent processing of the same PR or issue

Set `ordered: false` when your bot is stateless (sends notifications, records metrics, etc.).

---

## Step 1: Create a session-enabled queue

Ordered delivery requires FIFO / session support on the queue. Create the queue before registering the bot.

=== "Azure Service Bus"

    The queue **must** have `--requires-session true`:

    ```bash
    az servicebus queue create \
      --resource-group queue-keeper-rg \
      --namespace-name my-namespace \
      --name queue-keeper-my-bot \
      --requires-session true \
      --lock-duration PT5M \
      --default-message-time-to-live P14D \
      --max-delivery-count 10
    ```

    !!! warning
        If the queue was created without `--requires-session true`, Azure Service Bus silently ignores the session ID. Delete and recreate the queue with sessions enabled before registering an ordered bot.

=== "AWS SQS"

    Ordered delivery requires a **FIFO queue** (`.fifo` suffix):

    ```bash
    aws sqs create-queue \
      --queue-name queue-keeper-my-bot.fifo \
      --attributes FifoQueue=true,ContentBasedDeduplication=true,\
VisibilityTimeout=300,MessageRetentionPeriod=1209600
    ```

    Queue-Keeper sets the SQS `MessageGroupId` to the event `session_id`, which is what SQS FIFO uses to enforce per-group ordering.

---

## Step 2: Register the bot with `ordered: true`

```yaml
# bot-config.yaml
bots:
  - name: "my-stateful-bot"
    queue: "queue-keeper-my-bot"
    events:
      - "issues.*"
      - "pull_request.*"
    ordered: true
```

Restart Queue-Keeper after updating `bot-config.yaml`.

---

## Step 3: Write a session-aware receiver

=== "Python (azure-servicebus — Azure Service Bus)"

    ```python
    import json
    import logging
    import os

    from azure.servicebus import ServiceBusClient, NEXT_AVAILABLE_SESSION
    from azure.servicebus.exceptions import OperationTimeoutError

    logger = logging.getLogger(__name__)
    CONN_STR = os.environ["SERVICEBUS_CONNECTION_STRING"]
    QUEUE_NAME = "queue-keeper-my-bot"


    def process_event(event: dict) -> None:
        logger.info("[%s] %s.%s",
                    event["correlation_id"],
                    event["event_type"],
                    event.get("action", ""))


    def main() -> None:
        with ServiceBusClient.from_connection_string(CONN_STR) as client:
            while True:
                try:
                    with client.get_queue_session_receiver(
                        QUEUE_NAME,
                        session_id=NEXT_AVAILABLE_SESSION,
                        max_wait_time=30,
                    ) as receiver:
                        for msg in receiver:
                            try:
                                process_event(json.loads(str(msg)))
                                receiver.complete_message(msg)
                            except Exception as exc:
                                logger.error("Processing failed: %s", exc)
                                receiver.abandon_message(msg)
                except OperationTimeoutError:
                    continue
    ```

=== "C# (Azure.Messaging.ServiceBus)"

    ```csharp
    using Azure.Messaging.ServiceBus;
    using System.Text.Json;

    var client = new ServiceBusClient(connectionString);
    var processor = client.CreateSessionProcessor(queueName, new ServiceBusSessionProcessorOptions
    {
        MaxConcurrentSessions = 4,
        MaxConcurrentCallsPerSession = 1,
    });

    processor.ProcessMessageAsync += async args =>
    {
        var evt = JsonDocument.Parse(args.Message.Body.ToString()).RootElement;
        var correlationId = evt.GetProperty("correlation_id").GetString();
        var eventType = evt.GetProperty("event_type").GetString();
        Console.WriteLine($"[{correlationId}] {eventType}.{evt.GetProperty("action").GetString()}");
        await args.CompleteMessageAsync(args.Message);
    };

    processor.ProcessErrorAsync += args =>
    {
        Console.Error.WriteLine(args.Exception);
        return Task.CompletedTask;
    };

    await processor.StartProcessingAsync();
    Console.ReadKey();
    await processor.StopProcessingAsync();
    ```

=== "Python (boto3 — AWS SQS FIFO)"

    AWS SQS FIFO queues don't have a "session receiver" concept. Instead, messages in the same `MessageGroupId` are ordered automatically. Poll the queue and process messages sequentially:

    ```python
    import json
    import logging
    import os
    import boto3

    logger = logging.getLogger(__name__)
    sqs = boto3.client("sqs", region_name=os.environ["AWS_REGION"])
    QUEUE_URL = os.environ["SQS_QUEUE_URL"]


    def process_event(event: dict) -> None:
        logger.info("[%s] %s.%s",
                    event["correlation_id"],
                    event["event_type"],
                    event.get("action", ""))


    def main() -> None:
        while True:
            response = sqs.receive_message(
                QueueUrl=QUEUE_URL,
                MaxNumberOfMessages=1,
                WaitTimeSeconds=20,         # long-polling
                VisibilityTimeout=300,
            )
            for msg in response.get("Messages", []):
                body = json.loads(msg["Body"])
                try:
                    process_event(body)
                    sqs.delete_message(
                        QueueUrl=QUEUE_URL,
                        ReceiptHandle=msg["ReceiptHandle"]
                    )
                except Exception as exc:
                    logger.error("Processing failed: %s", exc)
                    # Message becomes visible again after VisibilityTimeout
    ```

    !!! note
        SQS FIFO enforces ordering per `MessageGroupId` (set by Queue-Keeper to the `session_id`). Messages in different groups are independent and may be polled concurrently by running multiple consumer instances.

---

## How sessions work in practice

Queue-Keeper sets the message's session/group identifier to the event's `session_id` field — for example `myorg/myrepo/pull_request/42`.

- **Azure Service Bus**: the `SessionId` message property is set. Azure guarantees all messages with the same `SessionId` are delivered to the same session receiver in enqueue order.
- **AWS SQS FIFO**: the `MessageGroupId` attribute is set. SQS guarantees ordering within the same group.

This means:

- Events for PR #42 always arrive in the order they were received by Queue-Keeper
- Events for PR #43 (different session) can be processed concurrently by a different receiver
- Your bot will never process two events for the same PR simultaneously

### Session / visibility lock

Your consumer holds a lock on the message while processing it. If processing takes longer than the lock duration (default: 5 minutes on Azure Service Bus, `VisibilityTimeout` on SQS), the lock expires and the message is re-delivered. Either:

- Keep processing well within the timeout
- Renew the lock if you need more time

---

## Troubleshooting

**"Cannot receive from a non-session-enabled queue"**
The queue was created without `--requires-session true`. Delete and recreate it with sessions enabled.

**Events arriving out of order**
You are using a standard (non-session) receiver on a session-enabled queue. Switch to `get_queue_session_receiver` / `CreateSessionProcessor`.

**Messages stuck, no consumer picks them up**
Another consumer holds the session lock. Check for crashed consumers that did not release their lock — sessions are released after the lock duration expires.
