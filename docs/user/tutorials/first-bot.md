# Build Your First Bot

This tutorial builds a minimal Python bot that receives GitHub `issues` events from Queue-Keeper and prints them to stdout. By the end you will have a working end-to-end pipeline: GitHub → Queue-Keeper → message queue → your bot.

!!! important "One webhook, many bots"
    GitHub webhooks are pointed at **Queue-Keeper**, not at individual bots. You configure one webhook in GitHub that sends all events to Queue-Keeper's endpoint. Queue-Keeper then routes each event to the queues of matching bots. Your bot only needs to subscribe to a queue — it never receives HTTP calls from GitHub directly.

**Time:** ~30 minutes
**Prerequisites:**

- Completed [Get Started](quickstart.md) (Queue-Keeper running locally)
- Python 3.9+
- A message queue: Azure Service Bus namespace (Standard or Premium tier) **or** AWS SQS
- Cloud CLI installed and authenticated (`az login` for Azure or `aws configure` for AWS)

---

## Step 1: Create the queue

Create a queue named `queue-keeper-demo-bot`. Because this tutorial uses unordered delivery (`ordered: false`), FIFO / sessions are not required.

=== "Azure Service Bus"

    ```bash
    az servicebus queue create \
      --resource-group my-rg \
      --namespace-name my-namespace \
      --name queue-keeper-demo-bot \
      --default-message-time-to-live P14D \
      --max-delivery-count 10
    ```

    Retrieve the connection string for local development:

    ```bash
    az servicebus namespace authorization-rule keys list \
      --resource-group my-rg \
      --namespace-name my-namespace \
      --name RootManageSharedAccessKey \
      --query primaryConnectionString \
      --output tsv
    ```

    Save the connection string — you will need it later.

=== "AWS SQS"

    ```bash
    aws sqs create-queue \
      --queue-name queue-keeper-demo-bot \
      --attributes VisibilityTimeout=300,MessageRetentionPeriod=1209600
    ```

    Retrieve the queue URL:

    ```bash
    aws sqs get-queue-url --queue-name queue-keeper-demo-bot
    ```

    Save the queue URL — you will need it later.

---

## Step 2: Update Queue-Keeper's configuration

Add the bot subscription to `bot-config.yaml`:

```yaml
bots:
  - name: "demo-bot"
    queue: "queue-keeper-demo-bot"
    events:
      - "issues.opened"
      - "issues.closed"
      - "issues.reopened"
    ordered: false
```

Update `service.yaml` to point Queue-Keeper at your queue backend:

=== "Azure Service Bus"

    ```yaml
    server:
      port: 8080
      host: "0.0.0.0"

    logging:
      level: "debug"
      json_format: false

    providers:
      - id: "github"
        require_signature: false

    queue:
      provider: azure_service_bus
      connection_string: "Endpoint=sb://my-namespace.servicebus.windows.net/;SharedAccessKeyName=RootManageSharedAccessKey;SharedAccessKey=..."
    ```

=== "AWS SQS"

    ```yaml
    server:
      port: 8080
      host: "0.0.0.0"

    logging:
      level: "debug"
      json_format: false

    providers:
      - id: "github"
        require_signature: false

    queue:
      provider: aws_sqs
      region: us-east-1
    ```

    The AWS SDK credential chain is used (environment variables, instance profile, etc.).

Restart Queue-Keeper with the updated configuration.

---

## Step 3: Install Python dependencies

```bash
python3 -m venv .venv
source .venv/bin/activate   # Windows: .venv\Scripts\activate
```

=== "Azure Service Bus"

    ```bash
    pip install azure-servicebus
    ```

=== "AWS SQS"

    ```bash
    pip install boto3
    ```

---

## Step 4: Write the bot

=== "Azure Service Bus"

    Create `bot.py`:

    ```python
    import json
    import logging
    import os

    from azure.servicebus import ServiceBusClient

    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s %(levelname)s %(name)s %(message)s"
    )
    logger = logging.getLogger("demo-bot")

    CONN_STR = os.environ["SERVICEBUS_CONNECTION_STRING"]
    QUEUE_NAME = "queue-keeper-demo-bot"


    def handle_event(event: dict) -> None:
        issue = event.get("payload", {}).get("issue", {})
        repo = event.get("payload", {}).get("repository", {}).get("full_name", "unknown")
        logger.info(
            "[%s] %s.%s — issue #%s in %s: %s",
            event["correlation_id"],
            event["event_type"],
            event.get("action"),
            issue.get("number", "?"),
            repo,
            issue.get("title", ""),
        )


    def main() -> None:
        logger.info("Bot starting, connecting to %s", QUEUE_NAME)
        with ServiceBusClient.from_connection_string(CONN_STR) as client:
            with client.get_queue_receiver(QUEUE_NAME, max_wait_time=30) as receiver:
                logger.info("Waiting for events…")
                for msg in receiver:
                    try:
                        handle_event(json.loads(str(msg)))
                        receiver.complete_message(msg)
                    except Exception as exc:
                        logger.error("Failed to process event: %s", exc)
                        receiver.abandon_message(msg)


    if __name__ == "__main__":
        main()
    ```

=== "AWS SQS"

    Create `bot.py`:

    ```python
    import json
    import logging
    import os
    import boto3

    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s %(levelname)s %(name)s %(message)s"
    )
    logger = logging.getLogger("demo-bot")

    sqs = boto3.client("sqs", region_name=os.environ.get("AWS_REGION", "us-east-1"))
    QUEUE_URL = os.environ["SQS_QUEUE_URL"]


    def handle_event(event: dict) -> None:
        issue = event.get("payload", {}).get("issue", {})
        repo = event.get("payload", {}).get("repository", {}).get("full_name", "unknown")
        logger.info(
            "[%s] %s.%s — issue #%s in %s: %s",
            event["correlation_id"],
            event["event_type"],
            event.get("action"),
            issue.get("number", "?"),
            repo,
            issue.get("title", ""),
        )


    def main() -> None:
        logger.info("Bot starting, polling %s", QUEUE_URL)
        while True:
            response = sqs.receive_message(
                QueueUrl=QUEUE_URL,
                MaxNumberOfMessages=10,
                WaitTimeSeconds=20,
                VisibilityTimeout=300,
            )
            for msg in response.get("Messages", []):
                try:
                    handle_event(json.loads(msg["Body"]))
                    sqs.delete_message(
                        QueueUrl=QUEUE_URL,
                        ReceiptHandle=msg["ReceiptHandle"]
                    )
                except Exception as exc:
                    logger.error("Failed to process event: %s", exc)
                    # Message becomes visible again after VisibilityTimeout


    if __name__ == "__main__":
        main()
    ```

---

## Step 5: Run the bot

=== "Azure Service Bus"

    ```bash
    export SERVICEBUS_CONNECTION_STRING="Endpoint=sb://..."
    python3 bot.py
    ```

    You should see:

    ```
    2026-05-09 10:00:00 INFO demo-bot Bot starting, connecting to queue-keeper-demo-bot
    2026-05-09 10:00:00 INFO demo-bot Waiting for events…
    ```

=== "AWS SQS"

    ```bash
    export SQS_QUEUE_URL="https://sqs.us-east-1.amazonaws.com/123456789012/queue-keeper-demo-bot"
    export AWS_REGION="us-east-1"
    python3 bot.py
    ```

    You should see:

    ```
    2026-05-09 10:00:00 INFO demo-bot Bot starting, polling https://sqs.us-east-1.amazonaws.com/...
    ```

---

## Step 6: Send a test event

In a separate terminal, send a simulated `issues.opened` webhook to Queue-Keeper:

```bash
curl -s -X POST http://localhost:8080/webhook/github \
  -H "Content-Type: application/json" \
  -H "X-GitHub-Event: issues" \
  -H "X-GitHub-Delivery: $(uuidgen || echo 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee')" \
  -d '{
    "action": "opened",
    "issue": {
      "number": 42,
      "title": "My first issue",
      "state": "open"
    },
    "repository": {
      "full_name": "myorg/myrepo",
      "owner": { "login": "myorg" },
      "name": "myrepo"
    }
  }'
```

Your bot should print:

```
2026-05-07 10:00:05 INFO demo-bot [<correlation_id>] issues.opened — issue #42 in myorg/myrepo: My first issue
```

---

## What you learned

- How to register a bot subscription in `bot-config.yaml`
- How to provision a message queue (Azure Service Bus or AWS SQS)
- How Queue-Keeper normalises a GitHub webhook into a `WrappedEvent` and delivers it to the queue
- How a Python bot consumes `WrappedEvent` messages
- That GitHub webhooks point to Queue-Keeper — not to individual bots

## Next steps

- [Use Ordered Delivery](../how-to/bot-developers/ordered-delivery.md) — receive events for the same issue or PR in order
- [Correlate Distributed Traces](../how-to/bot-developers/trace-correlation.md) — connect Queue-Keeper's trace IDs to your bot's spans
- [Deduplicate Replayed Events](../how-to/bot-developers/deduplicate-events.md) — handle redeliveries safely
- [Queue Message Format](../reference/queue-message-format.md) — full `WrappedEvent` schema reference
