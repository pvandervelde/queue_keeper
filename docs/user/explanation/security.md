# Security Model

Queue-Keeper implements defense-in-depth security. This page explains each layer of protection, the threats it addresses, and the design decisions behind it.

---

## Threat model

Queue-Keeper's primary concerns:

| Threat | Impact |
|---|---|
| **Forged webhooks** | Attacker injects arbitrary events, causing bots to take malicious actions |
| **Replay attacks** | Legitimate webhook is replayed to cause duplicate or out-of-sequence processing |
| **Denial of service** | Attacker overwhelms the service with junk requests |
| **Secret exfiltration** | Webhook signing secret is leaked, enabling webhook forgery |

---

## Layer 1: Network

**TLS everywhere**

All traffic reaches Queue-Keeper over TLS. The API gateway terminates TLS and presents a valid certificate. Cipher suites are restricted to AEAD algorithms (TLS 1.2+). Queue-Keeper never accepts plaintext HTTP in production.

**DDoS protection**

The API gateway (e.g. Azure Front Door, AWS CloudFront, Cloudflare) provides volumetric DDoS attack mitigation before traffic reaches Queue-Keeper.

**Private networking**

In production deployments, the message queue and object storage backends are accessed over private network connections (e.g. VNet private endpoints on Azure, VPC endpoints on AWS). Queue-Keeper never traverses the public internet to reach its backend services.

---

## Layer 2: Request authentication

**HMAC-SHA256 signature validation**

Every GitHub webhook carries an `X-Hub-Signature-256` header — an HMAC-SHA256 of the raw request body keyed with the webhook secret. Queue-Keeper validates this signature before processing the payload.

Key properties of the validation:

- **Constant-time comparison**: The calculated signature is compared to the received signature using a constant-time byte comparison. This prevents timing attacks in which an attacker probes the comparison one byte at a time.
- **Fail fast**: Signature failures return `400 Bad Request` immediately, without touching Blob Storage or Service Bus.
- **Raw body**: The HMAC is computed over the exact bytes received on the wire, before any JSON parsing. This prevents attacks that exploit parser differences.

Signature validation is configured per provider. For the GitHub built-in provider, set `require_signature: true` in `service.yaml` (strongly recommended for production).

---

## Layer 3: Rate limiting

**IP-based authentication failure tracking**

Queue-Keeper tracks signature validation failures per source IP using a sliding window. An IP that generates more than 10 authentication failures within a 5-minute window is blocked with `429 Too Many Requests`.

This prevents credential brute-forcing and slows down probing attacks. Legitimate webhook sources (GitHub's delivery infrastructure) use a small, well-known set of IP ranges and should never trigger the limit under normal operation.

---

## Layer 4: Secret management

**Secrets never touch disk**

In production, webhook secrets are stored in a managed secret store (e.g. Azure Key Vault, AWS Secrets Manager, HashiCorp Vault). Queue-Keeper fetches secrets at startup using workload identity (managed identity on Azure, IAM role on AWS) — no credentials stored in configuration files, environment variables, or container images.

**Cached with TTL**

Secrets are cached in memory (default TTL: 5 minutes) to avoid Key Vault latency on every request. On cache expiry, secrets are refreshed in the background. If Key Vault is unavailable, the cached value continues to be used (with a circuit breaker eventually opening if outage persists).

**Redacted from logs**

Secret values are never written to log output. The `Debug` implementation for types containing secrets renders them as `<REDACTED>`.

**Key rotation**

Because secrets are fetched from the secret store by name, rotating a secret involves:

1. Set a new version in the secret store (old version remains valid until deleted)
2. Wait for Queue-Keeper's cache to expire (or restart for immediate refresh)
3. Update the secret in GitHub

See [Rotate Secrets](../how-to/operators/rotate-secrets.md) for the full procedure.

---

## Layer 5: Payload handling

**Maximum payload size**

Payloads exceeding the configured `server.max_body_size` (default: 10 MB) are rejected with `413 Payload Too Large`. This prevents memory exhaustion attacks using oversized bodies. Adjust `max_body_size` in `service.yaml` if your provider sends larger payloads.

**No code execution**

Webhook payloads are parsed as JSON and stored. Queue-Keeper never evaluates payload content as code or template expressions.

**Immutable audit trail**

Every webhook is stored in object storage at an immutable path (`{year}/{month}/{day}/{event_id}.json`) upon receipt. The raw bytes are stored before any processing, so the audit record represents exactly what was received. Object storage is configured to deny public read access.

!!! note "Best-effort storage"
    Object storage writes are best-effort and do not block event routing. If the storage backend is unavailable, the event is still delivered to the bot queue but the audit record for that event is lost. See [Reliability — graceful degradation](reliability.md#graceful-degradation) for details.

---

## What Queue-Keeper does not protect against

**Compromised GitHub credentials**: If an attacker has write access to a repository, they can push legitimate-looking events. Queue-Keeper validates the webhook signature (i.e. the event came from GitHub) but cannot verify that the user action which triggered the event was authorised.

**Secrets in payload content**: GitHub webhook payloads may contain sensitive data (commit messages, issue body, environment names). Queue-Keeper stores raw payloads in Blob Storage. Ensure that access to the storage container is appropriately restricted.

**Bot-level vulnerabilities**: Queue-Keeper's security boundary ends at the queue. If your bot's processing logic has vulnerabilities, those are outside Queue-Keeper's scope.
