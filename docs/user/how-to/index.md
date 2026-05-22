# How-to Guides

How-to guides are problem-oriented. They assume you know what goal you want to achieve and show you how to accomplish it. Unlike tutorials, they do not hold your hand through every decision — they focus on the steps needed to solve a specific problem.

## For Operators

Operators deploy and run Queue-Keeper in production environments.

| Guide | When to use it |
|---|---|
| [Deploy with Docker](operators/deploy-docker.md) | Running Queue-Keeper on a single host or in a compose stack |
| [Deploy on Kubernetes](operators/deploy-kubernetes.md) | Running Queue-Keeper in a Kubernetes cluster |
| [Configure Azure Services](operators/configure-azure.md) | Provisioning Service Bus, Blob Storage, and Key Vault on Azure |
| [Configure AWS Services](operators/configure-aws.md) | Provisioning SQS, S3, and Secrets Manager on AWS |
| [Add a Bot Subscription](operators/add-bot-subscription.md) | Connecting a new downstream bot to Queue-Keeper |
| [Configure Webhook Providers](operators/configure-providers.md) | Setting up GitHub or generic (Jira, GitLab, Slack) providers |
| [Replay Events](operators/replay-events.md) | Reprocessing past events from Blob Storage |
| [Rotate Secrets](operators/rotate-secrets.md) | Updating GitHub webhook secrets without downtime |
| [Monitor the Service](operators/monitor.md) | Querying metrics, health checks, and setting up alerts |

## For Bot Developers

Bot developers write the downstream automation services that consume events.

| Guide | When to use it |
|---|---|
| [Register a Bot](bot-developers/register-bot.md) | Adding a new bot subscription to Queue-Keeper |
| [Use Ordered Delivery](bot-developers/ordered-delivery.md) | Ensuring events for the same PR or issue arrive in order |
| [Correlate Distributed Traces](bot-developers/trace-correlation.md) | Connecting your bot's spans to the Queue-Keeper trace |
| [Handle Dead-Letter Messages](bot-developers/handle-dead-letters.md) | Inspecting and reprocessing failed message deliveries |
| [Deduplicate Replayed Events](bot-developers/deduplicate-events.md) | Safely handling redeliveries from retries or replays |
