# ADR-0001: Provider URL Routing Strategy

## Status

Accepted

## Date

2025-01

## Context

Queue-Keeper must receive webhooks from multiple source systems (GitHub, GitLab,
Jira, Slack, etc.) over a single public HTTPS endpoint while keeping each
provider's processing logic independent and extensible.

The routing mechanism must:

1. Identify the originating provider from every incoming request.
2. Dispatch to the correct [`WebhookProcessor`] implementation without coupling
   the HTTP layer to any provider-specific logic.
3. Allow new providers to be added without modifying the routing code.
4. Preserve backward compatibility for the existing GitHub integration.
5. Return clear HTTP error responses (404, 400, 500) for unknown or
   misconfigured providers.

### Options Considered

| Option | Pros | Cons |
|--------|------|------|
| **A. Single endpoint `/webhook` + `X-Provider` header** | One URL to expose | Header coupling, non-standard |
| **B. Parameterised URL `/webhook/{provider}`** | Explicit, RESTful, easily firewall-routable | Requires registry lookup per request |
| **C. Separate sub-paths `/github/webhook`, `/jira/webhook`** | Very explicit | Hard-coded URL map, requires deploy changes for new providers |
| **D. Query parameter `/webhook?provider=github`** | Simple | Query params are not path-routable at load-balancer level |

## Decision

**Option B: Parameterised URL `/webhook/{provider}`.**

Every incoming POST request is routed to `POST /webhook/{provider}` where
`{provider}` is a URL-safe ASCII identifier (`[a-z0-9\-_]+`, non-empty).

A [`ProviderRegistry`] holds a `HashMap<ProviderId, Arc<dyn WebhookProcessor>>`
populated at startup. The `{provider}` path segment is validated against
[`ProviderId`] constraints and then looked up in the registry.

- **Unknown provider → 404 Not Found** — the provider is not configured.
- **Known provider, bad headers → 400 Bad Request** — the payload is invalid.
- **Known provider, valid request → 202 Accepted** — processing is enqueued.

### GitHub Backward Compatibility

The canonical GitHub provider ID is `"github"` and it is always registered,
even when no explicit `providers:` configuration entry is present — the startup
code inserts it with default settings if absent.

This means all existing integrations pointing at `/webhook/github` continue to
work without configuration changes.

### Generic Providers and Header Parsing

Standard GitHub providers require the `X-GitHub-Event` and `X-GitHub-Delivery`
headers. Non-GitHub providers do not send these headers.

To avoid rejecting generic provider requests at the header-parsing stage, the
handler detects whether the incoming `{provider}` matches any entry in
`generic_providers` and applies a relaxed header parser
(`WebhookHeaders::from_http_headers_relaxed`) that:

- Defaults `event_type` to `"webhook"` when `X-GitHub-Event` is absent.
- Auto-generates a UUID `delivery_id` when `X-GitHub-Delivery` is absent.
- Passes all raw HTTP headers through to the processor via
  `WebhookRequest.raw_headers` for provider-specific lookup logic.

## Consequences

### Positive

- Provider identity is visible in access logs and load-balancer routing rules
  without inspecting request bodies or headers.
- Adding a new provider requires only a configuration entry, not a code or
  deployment change.
- The registry pattern enables unit testing of the routing logic in isolation.
- Each provider's processing is fully isolated from others.

### Negative

- Webhook source URL changes require reconfiguring the upstream sender
  (e.g., GitHub settings page).
- The `{provider}` segment is exposed in server logs; operators must be aware
  of this when handling sensitive provider IDs.

### Neutral

- The `ProviderRegistry` is populated synchronously at startup; any
  configuration errors cause the process to exit with code 3 before accepting
  traffic.

## Implementation

Key types:

| Type | Location |
|------|----------|
| `ProviderId` | `crates/queue-keeper-api/src/provider_registry.rs` |
| `ProviderRegistry` | `crates/queue-keeper-api/src/provider_registry.rs` |
| `WebhookHeaders::from_http_headers` | `crates/queue-keeper-core/src/webhook/mod.rs` |
| `WebhookHeaders::from_http_headers_relaxed` | `crates/queue-keeper-core/src/webhook/mod.rs` |
| `handle_provider_webhook` | `crates/queue-keeper-api/src/lib.rs` |
