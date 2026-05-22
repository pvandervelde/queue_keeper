# Providers and Processing Modes

Queue-Keeper accepts webhooks from multiple sources. This page explains the two kinds of provider — the built-in GitHub provider and generic providers — and the two processing modes that determine how payloads are transformed before leaving Queue-Keeper.

---

## The built-in GitHub provider

GitHub is Queue-Keeper's primary built-in provider. It has first-class support for:

- **HMAC-SHA256 signature validation** using the `X-Hub-Signature-256` header
- **Full event normalisation** into the `WrappedEvent` schema with computed `session_id`, extracted `event_type`, and extracted `action`
- **Session ID generation** from all recognised GitHub entity types (pull requests, issues, branches, releases, and more)

The built-in provider is registered at `/webhook/github`.

!!! important "GitHub webhook points to Queue-Keeper, not to individual bots"
    In GitHub's webhook settings, the payload URL must be Queue-Keeper's endpoint — for example `https://queue-keeper.example.com/webhook/github`. A single webhook covers all events for that repository or organisation. Queue-Keeper then fans out each event to the queues of every matching bot. Individual bots do not have their own webhook URLs.

The GitHub provider always produces `WrappedEvent` messages. Use the bot subscriptions in `bot-config.yaml` to control which bots receive which events.

---

## Generic providers

Generic providers let you connect any webhook source — Jira, GitLab, Slack, PagerDuty, or a custom internal system — without writing Rust code. Each generic provider is fully configuration-driven via `service.yaml`.

A generic provider is registered at `/webhook/{provider_id}`, where `provider_id` is the `provider_id` you choose in the configuration.

### Configuration-driven flexibility

Generic providers are flexible in how they extract event type identifiers:

- **From a header**: `X-Atlassian-Event`, `X-Gitlab-Event`, etc.
- **From a JSON body field**: a path into the webhook payload JSON

Signature validation is optional and supports SHA-256, SHA-1, and plain shared-secret comparison.

---

## Processing modes

Each generic provider operates in one of two modes. The GitHub built-in provider always uses the `wrap` equivalent.

### Wrap mode

The webhook payload is parsed and wrapped inside a `WrappedEvent` envelope. The resulting message is routed to all bot queues that match the event type through the standard bot subscription mechanism.

**Use wrap mode when:**

- You want Queue-Keeper to handle fan-out to multiple bots
- You want session-based ordering (note: `session_id` is `null` for non-GitHub events in wrap mode)
- Your bots are already built to consume `WrappedEvent` messages
- You want consistent message format across multiple webhook sources

**Message format:** JSON `WrappedEvent` (see [Queue Message Format](queue-message-format.md))

### Direct mode

The raw webhook bytes are placed directly onto a specific queue without any parsing, transformation, or routing. No bot subscription evaluation occurs.

**Use direct mode when:**

- Your bot already speaks the native provider format (e.g. Jira webhook JSON)
- You want the lowest possible latency — no JSON parsing, no field extraction
- You have exactly one consumer for this webhook source
- The webhook format is complex or non-standard and transformation would be lossy

**Message format:** Raw webhook body bytes

```
             ┌─────────────┐
             │ Queue-Keeper│
Webhook ────▶ │             │──── WrappedEvent ────▶  Bot queues (via subscriptions)
             │  wrap mode  │
             └─────────────┘

             ┌─────────────┐
             │ Queue-Keeper│
Webhook ────▶ │             │──── Raw bytes ──────▶  Single target queue
             │ direct mode │
             └─────────────┘
```

---

## Choosing between wrap and direct

| Question | Answer → Use |
|---|---|
| Do multiple bots need this event? | **Wrap** — fan-out to multiple queues |
| Does the bot need `correlation_id` propagated? | **Wrap** — it's set in `WrappedEvent` |
| Does the bot already parse the native format? | **Direct** — avoid double-parse |
| Do you need session-based ordering? | **Wrap** (GitHub) / **Wrap with null session** (generic) |
| Lowest possible overhead is critical? | **Direct** |

---

## Multiple providers

You can register multiple providers of different types simultaneously. Queue-Keeper evaluates each incoming request against its registered provider ID in the URL path and routes it accordingly:

```
POST /webhook/github   → built-in GitHub provider  → WrappedEvent fan-out
POST /webhook/jira     → generic, direct mode       → single Jira queue
POST /webhook/gitlab   → generic, wrap mode         → WrappedEvent fan-out
```
