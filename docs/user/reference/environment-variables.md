# Environment Variables

Queue-Keeper reads the following environment variables. Most settings are also configurable via `service.yaml`; the environment variable takes precedence when both are set.

---

## Core

| Variable | Description | Example |
|---|---|---|
| `QUEUE_KEEPER_CONFIG` | Path to `service.yaml`. Equivalent to `--config` CLI flag. | `/config/service.yaml` |
| `QUEUE_KEEPER_LOG_LEVEL` | Override the `logging.level` value from `service.yaml`. | `debug` |

---

## Server

These variables override the corresponding `server.*` fields in `service.yaml`.

| Variable | Description | Default |
|---|---|---|
| `QUEUE_KEEPER_PORT` | HTTP port to listen on | `8080` |
| `QUEUE_KEEPER_HOST` | Interface to bind | `0.0.0.0` |

---

## Development overrides

These variables are provided as a convenience for local development without a Key Vault. **Do not use them in production** — they expose secrets in the process environment and in `docker inspect` output.

| Variable | Description |
|---|---|
| `QUEUE_KEEPER_GITHUB_SECRET` | Webhook secret for the built-in GitHub provider when `secret.type: literal` is not appropriate |

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
