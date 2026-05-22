# Environment Variables

Queue-Keeper reads the following environment variables. Most settings are also configurable via `service.yaml`; environment variables take precedence when both are set.

---

## Core

| Variable | Description | Example |
|---|---|---|
| `QUEUE_KEEPER_CONFIG` | Path to `service.yaml`. Equivalent to `--config` CLI flag. | `/config/service.yaml` |

---

## Configuration field overrides (`QK__` prefix)

Any field in `service.yaml` can be overridden by setting an environment variable using the pattern:

```
QK__<SECTION>__<FIELD>=<value>
```

The prefix is `QK__` (uppercase, double-underscore separator). Each nesting level is also separated by double underscores. The table below shows common overrides:

| Variable | Equivalent `service.yaml` field | Default |
|---|---|---|
| `QK__SERVER__PORT` | `server.port` | `8080` |
| `QK__SERVER__HOST` | `server.host` | `0.0.0.0` |
| `QK__SERVER__TIMEOUT_SECONDS` | `server.timeout_seconds` | `30` |
| `QK__LOGGING__LEVEL` | `logging.level` | `info` |
| `QK__LOGGING__JSON_FORMAT` | `logging.json_format` | `false` |
| `QK__SECURITY__ENABLE_RATE_LIMITING` | `security.enable_rate_limiting` | `true` |
| `QK__SECURITY__ADMIN_API_KEY` | `security.admin_api_key` | — |

!!! warning "Secrets in environment variables"
    Do not store `QK__SECURITY__ADMIN_API_KEY` or other secrets in a Dockerfile or compose file committed to source control. Inject them at runtime via your orchestrator's secrets mechanism (e.g. Kubernetes Secrets, Docker secrets, Azure Key Vault CSI driver).

---

## Azure SDK variables

Queue-Keeper uses the Azure SDK's default credential chain for managed identity authentication. The following variables are recognised by the Azure SDK (not Queue-Keeper itself) and may be useful for local development:

| Variable | Description |
|---|---|
| `AZURE_CLIENT_ID` | Client ID for a user-assigned managed identity or service principal |
| `AZURE_CLIENT_SECRET` | Client secret for service principal authentication |
| `AZURE_TENANT_ID` | Azure Active Directory tenant ID |
| `AZURE_SUBSCRIPTION_ID` | Azure subscription ID |

For local development, run `az login` before starting Queue-Keeper. The SDK's `DefaultAzureCredential` will use your `az` CLI session automatically.

---

## OpenTelemetry variables

Queue-Keeper exports traces and metrics when OpenTelemetry is configured. The following standard OTLP variables are recognised:

| Variable | Description | Example |
|---|---|---|
| `OTEL_EXPORTER_OTLP_ENDPOINT` | OTLP endpoint for traces and metrics | `http://otel-collector:4317` |
| `OTEL_SERVICE_NAME` | Service name in trace spans | `queue-keeper` |
| `OTEL_RESOURCE_ATTRIBUTES` | Additional resource attributes | `deployment.environment=production` |

!!! note
    If `OTEL_EXPORTER_OTLP_ENDPOINT` is not set, Queue-Keeper emits trace events to the log output only.
