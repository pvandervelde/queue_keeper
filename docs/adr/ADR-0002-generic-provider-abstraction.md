# ADR-0002: Generic Provider Abstraction

## Status

Accepted

## Date

2025-01

## Context

Many organisations use webhook-based integrations from sources other than GitHub
(e.g., Jira, Slack, GitLab, Bitbucket, custom internal tools). Adding a new
provider previously required:

1. Writing a new Rust `WebhookProcessor` implementation.
2. Updating the provider registry wiring.
3. Rebuilding and redeploying the service.

This made non-GitHub integrations expensive to add and maintained a high
coupling between configuration and code.

The requirements for the generic provider feature are:

- New webhook sources can be added via YAML configuration only — no Rust code.
- Each provider can be configured to forward payloads verbatim (**direct**
  mode) or normalise them into the standard [`WrappedEvent`] format (**wrap**
  mode).
- Per-provider HMAC or bearer-token signature validation is supported.
- Field extraction (event type, entity, action, repository) can be mapped to
  JSON paths or HTTP headers.
- The solution must be extensible without breaking the existing GitHub provider.

### Options Considered

| Option | Description | Decision |
|--------|-------------|---------|
| **A. Configuration-driven generic processor** | A `GenericWebhookProvider` driven by `GenericProviderConfig` YAML, supporting direct and wrap modes, field extraction via `FieldSource`, and pluggable signature validation. | **Chosen** |
| **B. Scripting (Lua/WASM)** | Embed a scripting runtime to allow arbitrary processing logic. | Rejected – high complexity, security surface, maintenance burden. |
| **C. Per-provider plugin DLLs** | Load provider logic from shared libraries. | Rejected – fragile, OS-dependent, requires containerised rebuilds. |
| **D. External transformation service** | Route through a gateway that transforms payloads. | Rejected – additional operational component, network round-trips, latency. |

## Decision

**Option A: Configuration-driven `GenericWebhookProvider`.**

### Processing Modes

Two modes are supported, selected per-provider via `processing_mode`:

| Mode | Behaviour | YAML value |
|------|-----------|-----------|
| **Direct** | Forward raw request body bytes to a configured Azure Service Bus queue. Metadata (provider ID, content type, delivery ID) is attached as message properties. | `direct` |
| **Wrap** | Parse the JSON payload, extract structured fields (`repository_path`, `entity_path`, `action_path`), and produce a [`WrappedEvent`] for downstream bots using the standard routing rules. | `wrap` |

**Direct mode** is recommended for providers whose payload schema is either
proprietary or consumed as-is by a downstream service without further routing.

**Wrap mode** is recommended when the downstream bots already process
[`WrappedEvent`]-compatible messages and the provider delivers JSON payloads
with extractable fields.

### Signature Validation

Optional per-provider HMAC or bearer-token validation is configured via
`signature:` and `webhook_secret:`:

| Algorithm | YAML value | Notes |
|-----------|-----------|-------|
| HMAC-SHA256 | `hmac_sha256` | Standard; GitHub and Stripe style |
| HMAC-SHA1 | `hmac_sha1` | Legacy support |
| Bearer token | `bearer_token` | Jira-style shared token |

The `webhook_secret.type` field names the secret source:

- `literal`: Hard-coded value — development and CI only.
- `key_vault`: Azure Key Vault — required for production.

When `signature:` is set but no `webhook_secret:` is configured, or the
`key_vault` source is used in a release where Key Vault is not yet wired, the
service logs a `WARN` and skips validation. This is a deliberate fail-open
default to avoid breaking deployments during migration.

### Field Sources (`FieldSource`)

Fields can be read from multiple locations:

| Source | Description | Example |
|--------|-------------|---------|
| `header` | HTTP header value (case-insensitive) | `source: header / name: X-Event-Type` |
| `json_path` | Dot-separated path into the JSON body | `source: json_path / path: object.type` |
| `static` | Compile-time constant | `source: static / value: "webhook"` |
| `auto_generate` | Server-assigned UUID (ULID for delivery ID) | `source: auto_generate` |

## Consequences

### Positive

- New webhook sources can be onboarded in minutes with no code changes.
- Both raw-forwarding and structured-normalisation workloads are supported.
- Signature validation is consistent with GitHub-style HMAC across all providers.
- The `GenericWebhookProvider` is a first-class `WebhookProcessor` and
  participates in all existing observability hooks.

### Negative

- Complex YAML configurations can be harder to read and debug than Rust code.
- `direct` mode loses structured routing — all events go to one queue.
- `wrap` mode requires the provider's JSON schema to be predictable enough
  to extract fields reliably.

### Neutral

- Key Vault–backed secrets for generic providers are not yet implemented. Until
  they are, only `literal` secrets are supported, which limits production use
  of signature validation for generic providers.

## Implementation

Key types:

| Type | Location |
|------|----------|
| `GenericProviderConfig` | `crates/queue-keeper-core/src/webhook/generic_provider.rs` |
| `GenericWebhookProvider` | `crates/queue-keeper-core/src/webhook/generic_provider.rs` |
| `ProcessingMode` | `crates/queue-keeper-core/src/webhook/generic_provider.rs` |
| `FieldSource` | `crates/queue-keeper-core/src/webhook/generic_provider.rs` |
| `SignatureConfig` | `crates/queue-keeper-core/src/webhook/generic_provider.rs` |
| `WebhookSecretConfig` | `crates/queue-keeper-core/src/webhook/generic_provider.rs` |
| `LiteralSignatureValidator` | `crates/queue-keeper-service/src/signature_validator.rs` |
| `build_validator_from_generic_config` | `crates/queue-keeper-service/src/main.rs` |

See [docs/configuration.md](../configuration.md) for complete YAML examples.
