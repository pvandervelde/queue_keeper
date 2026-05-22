# Queue-Keeper

Queue-Keeper is a Rust service that sits between GitHub and your automation bots. It receives GitHub webhook events, validates their signatures, persists raw payloads, and routes normalized events to your bots' queues with guaranteed delivery order.

```
GitHub ──webhook──▶ Queue-Keeper ──WrappedEvent──▶ Queue ──▶ Your Bot
              (validate · persist · normalise · route)
```

## Who is this documentation for?

**[Operators](how-to/operators/deploy-docker.md)** run Queue-Keeper in production. They configure the service, manage webhook provider credentials, provision Azure infrastructure, and keep the service healthy.

**[Bot developers](how-to/bot-developers/register-bot.md)** write the downstream automation services that consume events from Queue-Keeper. They subscribe to specific event types, receive ordered `WrappedEvent` messages, and act on them.

## Where to start

New to Queue-Keeper? Start with the tutorials — they walk you through your first successful deployment and your first bot, step by step:

- [Get Started](tutorials/quickstart.md) — run Queue-Keeper locally and process your first webhook in under 15 minutes
- [Build Your First Bot](tutorials/first-bot.md) — write a minimal Python consumer that responds to GitHub events

Already know what you need to do? Jump straight to the how-to guides:

- [Deploy with Docker](how-to/operators/deploy-docker.md)
- [Add a Bot Subscription](how-to/operators/add-bot-subscription.md)
- [Use Ordered Delivery](how-to/bot-developers/ordered-delivery.md)

Need to look something up? See the reference documentation:

- [Configuration reference](reference/configuration.md)
- [HTTP API reference](reference/api.md)
- [CLI reference](reference/cli.md)
- [Queue message format](reference/queue-message-format.md)

Want to understand how things work? Read the explanations:

- [Architecture](explanation/architecture.md)
- [Ordering and sessions](explanation/ordering-sessions.md)
- [Security model](explanation/security.md)
