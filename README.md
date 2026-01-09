# Queue-Keeper

GitHub webhook event processor with ordered delivery to downstream automation bots.

## Overview

Queue-Keeper is a Rust-based webhook intake and routing service that serves as the central entrypoint for GitHub webhooks. It validates, normalizes, persists, and routes webhook events to downstream automation bots with guaranteed ordering and reliability.

## Features

- **Webhook Validation** - Verify GitHub webhook signatures (HMAC-SHA256)
- **Event Persistence** - Store raw payloads in Azure Blob Storage for audit/replay
- **Event Normalization** - Transform GitHub webhooks into standardized event schema
- **Routing & Distribution** - Send events to configured bot queues with proper ordering
- **Reliability** - Implement retries, dead letter queues, and replay mechanisms
- **Observability** - Comprehensive logging, metrics, and distributed tracing

## Quick Start

### Using Docker

```bash
# Pull the latest image
docker pull ghcr.io/pvandervelde/queue-keeper:latest

# Run the container
docker run -p 8080:8080 \
  -e QUEUE_KEEPER_PORT=8080 \
  -e GITHUB_WEBHOOK_SECRET=your-secret \
  ghcr.io/pvandervelde/queue-keeper:latest
```

### Building from Source

```bash
# Clone the repository
git clone https://github.com/pvandervelde/queue_keeper.git
cd queue_keeper

# Build all crates
cargo build --release --workspace

# Run the service
cargo run --release --package queue-keeper-service
```

### Using as a Library

Add to your `Cargo.toml`:

```toml
[dependencies]
queue-keeper-core = "0.1.0"
github-bot-sdk = "0.1.0"
queue-runtime = "0.1.0"
```

## Documentation

- **[Contributing Guide](CONTRIBUTING.md)** - Development setup, commit conventions, and release process
- **[Architecture](specs/README.md)** - System design and component interactions
- **[API Documentation](https://docs.rs/queue-keeper-core)** - Rustdoc for all public APIs

## Project Structure

- `queue-keeper-core` - Core domain logic and traits
- `queue-keeper-service` - HTTP service implementation
- `queue-keeper-cli` - Command-line administrative interface
- `queue-keeper-api` - API types and handlers
- `github-bot-sdk` - GitHub API client library
- `queue-runtime` - Queue provider abstraction layer

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for:

- Development setup
- Commit message conventions
- Release process
- Testing guidelines
- Code organization

## Releases

Releases are automated through GitHub Actions:

- CLI binaries for Linux and Windows
- Docker images on GitHub Container Registry
- Crates published to crates.io

See the [Releases page](https://github.com/pvandervelde/queue_keeper/releases) for downloads.

## License

See [LICENSE](LICENSE) for details.
