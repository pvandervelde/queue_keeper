# Provider Integration Examples

This document provides complete, copy-pasteable configuration examples for integrating
Queue-Keeper with various webhook providers.

All examples assume Queue-Keeper is reachable at `https://queue-keeper.example.com`.
Replace secrets with values from Azure Key Vault in production (see
[Configuration Guide](configuration.md#generic_providers--configuration-driven-providers)).

---

## GitHub (Built-In Provider)

GitHub is the primary built-in provider with full event normalization into the standard
`EventEnvelope` format. Events are routed to bot queues according to the `bots`
subscription configuration.

### `service.yaml`

```yaml
providers:
  - id: "github"
    require_signature: true
    secret:
      type: key_vault
      secret_name: "github-webhook-secret"
    # Optionally restrict to specific event types:
    # allowed_event_types: ["push", "pull_request", "issues"]

key_vault:
  vault_url: "https://my-vault.vault.azure.net"
```

### GitHub Webhook Settings

- **Payload URL**: `https://queue-keeper.example.com/webhook/github`
- **Content type**: `application/json`
- **Secret**: value stored in Key Vault as `github-webhook-secret`
- **SSL verification**: Enabled

### Test Delivery

```bash
# Simulate a GitHub push event (development — literal secret)
SECRET="my-dev-secret"
PAYLOAD='{"ref":"refs/heads/main","repository":{"full_name":"myorg/myrepo"}}'
SIG="sha256=$(printf '%s' "$PAYLOAD" | openssl dgst -sha256 -hmac "$SECRET" | awk '{print $2}')"

curl -X POST https://queue-keeper.example.com/webhook/github \
  -H "Content-Type: application/json" \
  -H "X-GitHub-Event: push" \
  -H "X-GitHub-Delivery: $(uuidgen)" \
  -H "X-Hub-Signature-256: $SIG" \
  -d "$PAYLOAD"
```

---

## Jira (Generic Provider — Direct Mode)

Forwards raw Jira webhook JSON to an Azure Service Bus queue without transformation.
The downstream consumer receives the Jira payload directly.

### Use When

- Your bot is already built to consume the native Jira webhook format.
- You don't need standard bot-subscription routing.
- You want minimal latency (no JSON parsing or field extraction).

### `service.yaml`

```yaml
generic_providers:
  - provider_id: "jira"
    processing_mode: direct
    target_queue: "queue-keeper-jira"
    event_type_source:
      source: header
      name: "X-Atlassian-Event"
    signature:
      header_name: "X-Hub-Signature"
      algorithm: hmac_sha256
    webhook_secret:
      type: key_vault
      secret_name: "jira-webhook-secret"

key_vault:
  vault_url: "https://my-vault.vault.azure.net"
```

### Jira Webhook Settings

- **URL**: `https://queue-keeper.example.com/webhook/jira`
- In **Jira → System → Webhooks**, set the URL and the shared secret if using signature validation.

### Test Delivery

```bash
curl -X POST https://queue-keeper.example.com/webhook/jira \
  -H "Content-Type: application/json" \
  -H "X-Atlassian-Event: jira:issue_created" \
  -d '{"webhookEvent":"jira:issue_created","issue":{"id":"10001","key":"PROJ-1","fields":{"summary":"Example issue"}}}'
```

---

## GitLab (Generic Provider — Wrap Mode)

Normalises GitLab merge request and issue webhooks into the standard `WrappedEvent`
format so they can be routed to bots using the normal `bots` subscription configuration.

### Use When

- Your bots consume `WrappedEvent`-format messages (the same format GitHub events produce).
- You want repository/entity-based session ordering for GitLab events.

### `service.yaml`

```yaml
generic_providers:
  - provider_id: "gitlab"
    processing_mode: wrap
    event_type_source:
      source: header
      name: "X-Gitlab-Event"
    signature:
      header_name: "X-Gitlab-Token"
      algorithm: bearer_token
    webhook_secret:
      type: key_vault
      secret_name: "gitlab-webhook-token"
    field_extraction:
      # "owner/repo" path — GitLab uses "project.path_with_namespace"
      repository_path: "project.path_with_namespace"
      # Entity ID — MR/issue internal ID
      entity_path: "object_attributes.iid"
      # Action — "open", "merge", "close", etc.
      action_path: "object_attributes.action"

key_vault:
  vault_url: "https://my-vault.vault.azure.net"
```

### GitLab Webhook Settings

In **GitLab → Project → Settings → Webhooks**:

- **URL**: `https://queue-keeper.example.com/webhook/gitlab`
- **Secret token**: value stored in Key Vault as `gitlab-webhook-token`
- **Trigger events**: Select the events you want queued (e.g. Merge request events, Issue events)

### Bot Subscription Example

Because GitLab events are normalised into `WrappedEvent`, you can use the same
`bots` subscription to route them:

```yaml
bots:
  - name: "gitlab-mr-handler"
    queue: "queue-keeper-gitlab-bot"
    events: ["Merge Request Hook"]   # GitLab event type strings
    ordered: true
```

### Test Delivery

```bash
curl -X POST https://queue-keeper.example.com/webhook/gitlab \
  -H "Content-Type: application/json" \
  -H "X-Gitlab-Event: Merge Request Hook" \
  -H "X-Gitlab-Token: my-dev-token" \
  -d '{
    "project": {"path_with_namespace": "mygroup/myproject"},
    "object_attributes": {"iid": 1, "action": "open", "state": "opened"}
  }'
```

---

## Slack Events API (Generic Provider — Wrap Mode)

Routes Slack event payloads into the standard `WrappedEvent` format.

> **Note**: Slack's URL verification challenge (`type: url_verification`) is sent as
> a `POST` to confirm the endpoint. Queue-Keeper will forward this to the target queue.
> Your bot must respond to the challenge via a separate mechanism or expose its own
> HTTP endpoint.

### `service.yaml`

```yaml
generic_providers:
  - provider_id: "slack"
    processing_mode: wrap
    event_type_source:
      source: json_path
      path: "event.type"            # e.g. "message", "app_mention"
    delivery_id_source:
      source: auto_generate         # Slack has no delivery ID header
    # No signature configured — rely on Slack's URL verification challenge
    field_extraction:
      repository_path: "team_id"   # Slack workspace ID used as the "repository"
      entity_path: "event.channel" # Channel ID as entity
      action_path: "event.subtype"
```

### Slack App Settings

In **Slack API → Event Subscriptions**:

- **Request URL**: `https://queue-keeper.example.com/webhook/slack`
- Subscribe to the bot events you want (e.g. `message.channels`, `app_mention`).

### Test Delivery

```bash
curl -X POST https://queue-keeper.example.com/webhook/slack \
  -H "Content-Type: application/json" \
  -d '{
    "type": "event_callback",
    "team_id": "T12345678",
    "event": {
      "type": "app_mention",
      "channel": "C12345678",
      "text": "Hello <@U12345678>"
    }
  }'
```

---

## Custom Internal Tool (Generic Provider — Direct Mode, No Signature)

For internal tools where you control the sender and don't need signature validation.

### `service.yaml`

```yaml
generic_providers:
  - provider_id: "internal-ci"
    processing_mode: direct
    target_queue: "queue-keeper-ci-events"
    event_type_source:
      source: json_path
      path: "event_type"
    delivery_id_source:
      source: json_path
      path: "build_id"
    # No signature — internal network trust model
```

### Test Delivery

```bash
curl -X POST https://queue-keeper.example.com/webhook/internal-ci \
  -H "Content-Type: application/json" \
  -d '{
    "event_type": "build.completed",
    "build_id": "build-20260408-001",
    "status": "success",
    "branch": "main"
  }'
```

---

## Multi-Provider Combined Example

A complete `service.yaml` serving GitHub, GitLab, and Jira simultaneously:

```yaml
server:
  host: "0.0.0.0"
  port: 8080

logging:
  level: "info"
  format: "json"

providers:
  - id: "github"
    require_signature: true
    secret:
      type: key_vault
      secret_name: "github-webhook-secret"

generic_providers:
  - provider_id: "gitlab"
    processing_mode: wrap
    event_type_source:
      source: header
      name: "X-Gitlab-Event"
    signature:
      header_name: "X-Gitlab-Token"
      algorithm: bearer_token
    webhook_secret:
      type: key_vault
      secret_name: "gitlab-webhook-token"
    field_extraction:
      repository_path: "project.path_with_namespace"
      entity_path: "object_attributes.iid"
      action_path: "object_attributes.action"

  - provider_id: "jira"
    processing_mode: direct
    target_queue: "queue-keeper-jira"
    event_type_source:
      source: header
      name: "X-Atlassian-Event"
    signature:
      header_name: "X-Hub-Signature"
      algorithm: hmac_sha256
    webhook_secret:
      type: key_vault
      secret_name: "jira-webhook-secret"

key_vault:
  vault_url: "https://my-vault.vault.azure.net"

queue:
  type: azure_service_bus
  namespace: "your-servicebus-namespace"
```

With this configuration, Queue-Keeper accepts traffic at:

- `POST /webhook/github` — GitHub events
- `POST /webhook/gitlab` — GitLab events (normalised to WrappedEvent)
- `POST /webhook/jira` — Jira events (forwarded raw to `queue-keeper-jira`)
