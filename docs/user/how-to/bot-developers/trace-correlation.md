# Correlate Distributed Traces

This guide shows how to extract the `correlation_id` from a `WrappedEvent` message and propagate it into your bot's spans so that a single trace covers the full journey from GitHub through Queue-Keeper to your bot.

---

## How correlation IDs flow

When Queue-Keeper receives a webhook it looks for an upstream trace in the following headers (first match wins):

1. `traceparent` — W3C Trace Context (recommended)
2. `X-Correlation-ID`
3. `X-Request-ID`

If none are present, Queue-Keeper generates a fresh UUID v4.

The extracted or generated value is stamped as `correlation_id` on every `WrappedEvent` or `DirectQueueMetadata` block placed on the queue. It also appears in every audit log entry produced during that request.

---

## Step 1: Extract `correlation_id` from the message

The value lives at the top level of the `WrappedEvent` JSON body:

```json
{
  "event_id": "01JQZM7XK4B3VYFNHD0G2T8P1X",
  "correlation_id": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
  ...
}
```

=== "Python"

    ```python
    import json

    def handle_message(msg) -> None:
        event = json.loads(str(msg))
        correlation_id = event["correlation_id"]
        event_id = event["event_id"]
    ```

=== "C#"

    ```csharp
    var evt = JsonDocument.Parse(args.Message.Body.ToString()).RootElement;
    var correlationId = evt.GetProperty("correlation_id").GetString();
    var eventId = evt.GetProperty("event_id").GetString();
    ```

---

## Step 2: Propagate the correlation ID in your bot's spans

The `correlation_id` is a W3C `traceparent` string when the originating request included one, and a UUID otherwise. Regardless of format, use it to mark your bot's spans so that they are searchable by the same identifier.

=== "Python (OpenTelemetry)"

    ```python
    from opentelemetry import trace
    from opentelemetry.trace.propagation.tracecontext import TraceContextTextMapPropagator

    tracer = trace.get_tracer("my-bot")
    propagator = TraceContextTextMapPropagator()


    def handle_event(event: dict) -> None:
        correlation_id = event["correlation_id"]

        # If correlation_id looks like a W3C traceparent, extract the context
        carrier = {"traceparent": correlation_id}
        ctx = propagator.extract(carrier=carrier)

        with tracer.start_as_current_span(
            "process_event",
            context=ctx,
            attributes={
                "qk.event_id": event["event_id"],
                "qk.event_type": event["event_type"],
                "qk.session_id": event.get("session_id", ""),
            },
        ):
            # ... your bot logic ...
            pass
    ```

=== "C# (OpenTelemetry)"

    ```csharp
    using OpenTelemetry;
    using OpenTelemetry.Context.Propagation;
    using System.Diagnostics;

    static readonly TextMapPropagator Propagator = Propagators.DefaultTextMapPropagator;
    static readonly ActivitySource ActivitySource = new("my-bot");

    void HandleEvent(JsonElement evt)
    {
        var correlationId = evt.GetProperty("correlation_id").GetString()!;
        var carrier = new Dictionary<string, string> { ["traceparent"] = correlationId };
        var parentContext = Propagator.Extract(default, carrier, (c, key) =>
            c.TryGetValue(key, out var v) ? new[] { v } : Array.Empty<string>());

        using var activity = ActivitySource.StartActivity(
            "process_event",
            ActivityKind.Consumer,
            parentContext.ActivityContext);

        activity?.SetTag("qk.event_id", evt.GetProperty("event_id").GetString());
        activity?.SetTag("qk.event_type", evt.GetProperty("event_type").GetString());

        // ... your bot logic ...
    }
    ```

---

## Step 3: Correlate with GitHub's delivery ID

For GitHub events, Queue-Keeper logs a structured entry pairing the GitHub `X-GitHub-Delivery` ID with the `correlation_id`:

```json
{
  "level": "info",
  "message": "github delivery correlation",
  "github_delivery_id": "12345678-1234-1234-1234-123456789012",
  "correlation_id": "00-4bf92f..."
}
```

This lets you cross-reference Queue-Keeper's processing logs with the delivery history in GitHub's webhook settings using either ID.

---

## Logging the correlation ID

Even without full distributed tracing, always log `correlation_id` alongside every event your bot processes. This enables grepping across Queue-Keeper and bot logs with a single identifier:

```python
logger.info(
    "Processing event",
    extra={
        "correlation_id": event["correlation_id"],
        "event_id": event["event_id"],
        "event_type": event["event_type"],
    }
)
```
