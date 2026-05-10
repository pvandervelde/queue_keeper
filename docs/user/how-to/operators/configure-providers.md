# Configure Webhook Providers

This guide shows how to configure Queue-Keeper to accept webhooks from GitHub and from generic providers (Jira, GitLab, Slack, and others). Providers are defined in `service.yaml`.

---

## GitHub (built-in provider)

GitHub is Queue-Keeper's primary built-in provider. It normalises GitHub webhook payloads into the `WrappedEvent` format with full session-ID generation.

!!! important "Webhook URL points to Queue-Keeper, not your bots"
    When configuring the webhook in GitHub, the payload URL must point to **Queue-Keeper** — for example `https://queue-keeper.example.com/webhook/github`. Queue-Keeper then routes received events to each registered bot's queue. Individual bots do **not** have their own webhook URLs and must not be registered directly in GitHub.

### `service.yaml` configuration

```yaml
providers:
  - id: "github"
    require_signature: true
    secret:
      type: key_vault
      secret_name: "github-webhook-secret"
```

### Required `service.yaml` companion

```yaml
key_vault:
  vault_url: "https://my-vault.vault.azure.net"
```

### GitHub webhook settings

In your GitHub repository or organisation's **Settings → Webhooks → Add webhook**:

| Field | Value |
|---|---|
| Payload URL | `https://queue-keeper.example.com/webhook/github` |
| Content type | `application/json` |
| Secret | The value stored in Key Vault as `github-webhook-secret` |
| SSL verification | Enabled |
| Which events | Select the events your bots need, or send everything |

### Verify delivery

GitHub's webhook settings page shows a list of recent deliveries. A `200 OK` response confirms Queue-Keeper received and processed the event.

---

## Generic providers

Generic providers let you connect any webhook source without writing Rust code. Each provider is configured with a unique `provider_id` and a processing mode.

### Processing modes

| Mode | Output | Use when |
|---|---|---|
| `wrap` | `WrappedEvent` JSON on the bot queues | You want standard routing, fan-out, and session ordering |
| `direct` | Raw webhook body bytes on a single queue | Your bot already speaks the native format and you want minimum latency |

### Jira — direct mode

Forward raw Jira payloads directly to a single queue without transformation:

```yaml
generic_providers:
  - provider_id: "jira"
    processing_mode: direct
    target_queue: "queue-keeper-jira"
    event_type_source:
      type: header
      name: "X-Atlassian-Event"
    signature:
      header_name: "X-Hub-Signature"
      algorithm: sha256
      secret:
        type: key_vault
        secret_name: "jira-webhook-secret"
```

Webhook URL to register in Jira: `https://queue-keeper.example.com/webhook/jira`

### GitLab — wrap mode

Normalise GitLab webhooks into `WrappedEvent` and route to bot queues:

```yaml
generic_providers:
  - provider_id: "gitlab"
    processing_mode: wrap
    event_type_source:
      type: header
      name: "X-Gitlab-Event"
    signature:
      header_name: "X-Gitlab-Token"
      algorithm: plain
      secret:
        type: key_vault
        secret_name: "gitlab-webhook-secret"
```

Webhook URL: `https://queue-keeper.example.com/webhook/gitlab`

### Slack — direct mode, no signature

```yaml
generic_providers:
  - provider_id: "slack"
    processing_mode: direct
    target_queue: "queue-keeper-slack"
    event_type_source:
      type: body_field
      field_path: "type"
```

!!! warning "Signature validation"
    Configuring a provider with no signature block (`signature:` omitted) accepts all requests to that endpoint without authentication. Only do this if the provider does not support HMAC signatures and the endpoint is otherwise protected (private network, IP allowlist).

---

## Literal secrets (development only)

For local development you can specify a webhook secret as a literal string instead of fetching it from Key Vault:

```yaml
providers:
  - id: "github"
    require_signature: true
    secret:
      type: literal
      value: "my-dev-secret"
```

!!! danger
    Literal secrets write the secret value to the configuration file on disk. Never use this in production.

---

## Multiple providers

You can register multiple providers of the same type as long as their `id` / `provider_id` values are unique:

```yaml
providers:
  - id: "github"
    require_signature: true
    secret:
      type: key_vault
      secret_name: "github-webhook-secret"

generic_providers:
  - provider_id: "jira"
    processing_mode: direct
    target_queue: "queue-keeper-jira"
    event_type_source:
      type: header
      name: "X-Atlassian-Event"

  - provider_id: "gitlab"
    processing_mode: wrap
    event_type_source:
      type: header
      name: "X-Gitlab-Event"
```
