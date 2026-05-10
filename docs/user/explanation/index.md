# Explanation

The explanation section builds background understanding of how Queue-Keeper works, why it is designed the way it is, and the trade-offs behind key decisions. Read this section when you want to understand the reasoning behind the system, not just how to use it.

| Article | What it explains |
|---|---|
| [Architecture](architecture.md) | System components, data flow, and the role each part plays |
| [Ordering and Sessions](ordering-sessions.md) | Why ordered delivery exists and how Azure Service Bus sessions make it work |
| [Providers and Processing Modes](providers.md) | The built-in GitHub provider, generic providers, wrap mode vs direct mode |
| [Security Model](security.md) | Webhook signature validation, rate limiting, and the defense-in-depth approach |
| [Reliability](reliability.md) | Retries, circuit breakers, dead-letter queues, and persistence |
