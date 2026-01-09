# Contributing to Queue-Keeper

Thank you for your interest in contributing to Queue-Keeper! This document provides guidelines and information for contributors.

## Table of Contents

- [Development Setup](#development-setup)
- [Commit Message Conventions](#commit-message-conventions)
- [Release Process](#release-process)
- [Testing](#testing)
- [Code Organization](#code-organization)
- [Pull Request Process](#pull-request-process)

## Development Setup

### Prerequisites

- **Rust**: 1.70 or later
- **Cargo**: Latest stable version
- **Git**: For version control

### Building the Project

```bash
# Clone the repository
git clone https://github.com/pvandervelde/queue_keeper.git
cd queue_keeper

# Build all crates
cargo build --workspace

# Run tests
cargo test --workspace

# Run linters
cargo clippy -- -D warnings
cargo fmt -- --check
```

### Project Structure

Queue-Keeper is organized as a Cargo workspace with multiple crates:

- `queue-keeper-core` - Core domain logic and traits
- `queue-keeper-service` - HTTP service implementation
- `queue-keeper-cli` - Command-line administrative interface
- `queue-keeper-api` - API types and handlers
- `github-bot-sdk` - GitHub API client library
- `queue-runtime` - Queue provider abstraction layer

## Commit Message Conventions

Queue-Keeper uses [Conventional Commits](https://www.conventionalcommits.org/) for automated changelog generation and semantic versioning.

### Commit Message Format

```
<type>(<scope>): <description>

[optional body]

[optional footer(s)]
</description>
```

### Commit Types

The commit type determines how the version is bumped:

| Type | Description | Version Bump | Example |
|------|-------------|--------------|---------|
| `feat` | New feature | **Minor** (0.1.0 → 0.2.0) | `feat(auth): add JWT token validation` |
| `fix` | Bug fix | **Patch** (0.1.0 → 0.1.1) | `fix(webhook): handle empty payload gracefully` |
| `perf` | Performance improvement | Patch | `perf(queue): optimize batch message processing` |
| `refactor` | Code refactoring | Patch | `refactor(storage): simplify blob upload logic` |
| `docs` | Documentation changes | None | `docs(readme): update installation instructions` |
| `test` | Test additions/changes | None | `test(auth): add JWT expiration tests` |
| `chore` | Build/tooling changes | None | `chore(deps): update tokio to 1.40` |
| `ci` | CI/CD changes | None | `ci(actions): add security audit workflow` |
| `style` | Code style changes | None | `style(format): run cargo fmt` |
| `build` | Build system changes | None | `build(docker): optimize layer caching` |

### Breaking Changes

To trigger a **major version bump** (0.1.0 → 1.0.0), add `BREAKING CHANGE:` in the commit footer or use `!` after the type:

```bash
# Method 1: Using footer
feat(api)!: change webhook endpoint path

BREAKING CHANGE: Webhook endpoint moved from /webhook to /api/v1/webhook

# Method 2: Using ! notation
feat(queue)!: remove deprecated send_sync method
```

### Scope Examples

Choose a scope that describes the affected component:

- `auth` - Authentication and authorization
- `webhook` - Webhook processing
- `queue` - Queue operations
- `storage` - Blob storage operations
- `monitoring` - Metrics and observability
- `cli` - Command-line interface
- `api` - HTTP API endpoints
- `github` - GitHub API integration
- `config` - Configuration management

### Example Commits

```bash
# Feature addition (minor bump)
feat(monitoring): add prometheus metrics endpoint

# Bug fix (patch bump)
fix(webhook): validate signature before processing

# Breaking change (major bump)
feat(api)!: require authentication for all endpoints

BREAKING CHANGE: All API endpoints now require bearer token authentication

# Documentation (no version bump)
docs(contributing): add commit message guidelines

# Refactoring (patch bump)
refactor(queue): extract session key generation to separate module

# Performance improvement (patch bump)
perf(storage): implement connection pooling for blob uploads
```

## Release Process

Queue-Keeper uses an automated release workflow based on conventional commits. The process is fully automated through GitHub Actions.

### Automated Release Workflow

1. **Merge to Master** → When commits are merged to the `master` branch, a GitHub Action automatically:
   - Analyzes conventional commit messages since the last release
   - Calculates the next semantic version based on commit types
   - Generates a changelog using `git-cliff`
   - Creates or updates a release PR with version bumps

2. **Review Release PR** → The release PR includes:
   - Updated version in all `Cargo.toml` files
   - Generated `CHANGELOG.md` with categorized changes
   - Summary of changes grouped by type (Features, Bug Fixes, etc.)

3. **Adjust Version (Optional)** → If you need a different version bump:
   - Comment on the release PR with: `/version <bump-type>`
   - Valid bump types:
     - `/version minor` - Upgrade from patch to minor (e.g., 0.1.1 → 0.2.0)
     - `/version major` - Upgrade from minor to major (e.g., 0.2.0 → 1.0.0)
   - The bot will validate and apply the version change
   - ✅ reaction = valid suggestion applied
   - ❌ reaction = invalid suggestion (downgrade or same level)

4. **Merge Release PR** → When the release PR is merged:
   - Git tag is created (e.g., `v1.2.3`)
   - GitHub Release is published with changelog
   - CLI binaries are built for Linux and Windows
   - Docker image is built and pushed to `ghcr.io/pvandervelde/queue-keeper`
   - Artifacts are attached to the release

### Version Bump Rules

The automated system follows these rules:

| Last Commit Type | Default Bump | Adjustable To |
|-----------------|--------------|---------------|
| `feat:` | Minor (0.1.0 → 0.2.0) | Major |
| `fix:`, `perf:`, `refactor:` | Patch (0.1.0 → 0.1.1) | Minor, Major |
| `feat!:` or `BREAKING CHANGE:` | Major (0.1.0 → 1.0.0) | - |

**Important**: Version suggestions only allow **upward** adjustments:

- ✅ Patch → Minor → Major
- ❌ Cannot downgrade (Minor → Patch)
- ❌ Cannot stay same level

### Release Artifacts

After a successful release, the following artifacts are available:

#### CLI Binaries

Download platform-specific binaries from the GitHub Releases page:

```bash
# Linux (x86_64)
https://github.com/pvandervelde/queue_keeper/releases/download/v{version}/queue-keeper-cli-linux-x86_64

# Windows (x86_64)
https://github.com/pvandervelde/queue_keeper/releases/download/v{version}/queue-keeper-cli-windows-x86_64.exe

# macOS users: Build from source
cargo install queue-keeper-cli
# or
git clone https://github.com/pvandervelde/queue_keeper.git
cd queue_keeper
cargo build --release --package queue-keeper-cli
```

#### Docker Images

Docker images are published to GitHub Container Registry:

```bash
# Specific version
docker pull ghcr.io/pvandervelde/queue-keeper:1.2.3

# Latest release
docker pull ghcr.io/pvandervelde/queue-keeper:latest

# Run the container
docker run -p 8080:8080 \
  -e QUEUE_KEEPER_PORT=8080 \
  ghcr.io/pvandervelde/queue-keeper:latest
```

#### Rust Crates

Individual crates are published to crates.io:

```bash
# Add to Cargo.toml
[dependencies]
queue-keeper-core = "1.2.3"
github-bot-sdk = "1.2.3"
queue-runtime = "1.2.3"
```

## Testing

### Running Tests

```bash
# Run all tests
cargo test --workspace

# Run tests for a specific crate
cargo test --package queue-keeper-core

# Run tests with output
cargo test --workspace -- --nocapture

# Run ignored tests
cargo test --workspace -- --ignored
```

### Test Organization

- **Unit tests**: In `<module>_tests.rs` files adjacent to source
- **Integration tests**: In `tests/` directories at crate root
- **End-to-end tests**: In `crates/queue-keeper-e2e-tests`

### Test Coverage Expectations

- Core business logic: 100% coverage
- Error paths: All error variants tested
- Edge cases: Boundary conditions covered
- Security-critical code: Exhaustive testing

## Code Organization

### Module Structure

```
src/
├── lib.rs              # Public API and module declarations
├── error.rs            # Error types
├── error_tests.rs      # Error tests
├── module1.rs          # Single-file module
└── module2/            # Multi-file module
    ├── mod.rs          # Public interface
    ├── mod_tests.rs    # Module tests
    ├── types.rs        # Domain types
    └── types_tests.rs  # Type tests
```

### Naming Conventions

- **Module names**: `snake_case` (e.g., `auth_provider`, `queue_client`)
- **Type names**: `PascalCase` (e.g., `GitHubAppId`, `InstallationToken`)
- **Function names**: `snake_case` (e.g., `get_installation_token`, `is_expired`)
- **Test files**: `<source_file>_tests.rs`
- **Test functions**: `test_<what_is_being_tested>`

### Documentation

All public APIs must have rustdoc comments with:

- Brief one-line summary
- Detailed explanation with behaviors and constraints
- Examples section with working code
- Errors section documenting failure conditions
- Panics section if applicable

Example:

```rust
/// Validates a GitHub webhook signature using HMAC-SHA256.
///
/// Compares the provided signature against a computed HMAC using the
/// configured webhook secret. Uses constant-time comparison to prevent
/// timing attacks.
///
/// # Examples
///
/// ```
/// use queue_keeper_core::webhook::validate_signature;
///
/// let secret = "my-webhook-secret";
/// let payload = b"webhook payload";
/// let signature = "sha256=abc123...";
///
/// assert!(validate_signature(secret, payload, signature).is_ok());
/// ```
///
/// # Errors
///
/// Returns `WebhookError::InvalidSignature` if:
/// - Signature format is invalid
/// - Computed signature doesn't match provided signature
pub fn validate_signature(
    secret: &str,
    payload: &[u8],
    signature: &str,
) -> Result<(), WebhookError> {
    // Implementation...
}
```

## Pull Request Process

### Before Creating a PR

1. **Branch Naming**:
   - Feature: `feature/task-X.Y-description`
   - Bug fix: `fix/issue-description`
   - **Never commit directly to `master`**

2. **Code Quality**:

   ```bash
   cargo fmt              # Format code
   cargo clippy           # Run linter
   cargo test --workspace # Run all tests
   ```

3. **Commit Messages**: Follow conventional commit format

### PR Guidelines

1. **Title**: Use conventional commit format
   - Example: `feat(monitoring): add prometheus metrics endpoint`

2. **Description**: Explain what and why
   - What problem does this solve?
   - How does it solve it?
   - Any breaking changes?
   - Related issues/PRs?

3. **Review Process**:
   - Ensure all CI checks pass
   - Address reviewer feedback
   - Keep commits atomic and well-described
   - Squash commits before merge if needed

### PR Checklist

- [ ] Code follows project conventions
- [ ] All tests pass locally
- [ ] Documentation updated (if needed)
- [ ] Commit messages follow conventional format
- [ ] No direct commits to `master`
- [ ] CI/CD checks pass

## Getting Help

- **Issues**: [GitHub Issues](https://github.com/pvandervelde/queue_keeper/issues)
- **Discussions**: [GitHub Discussions](https://github.com/pvandervelde/queue_keeper/discussions)
- **Specifications**: See `specs/`, `github-bot-sdk-specs/`, and `queue-runtime-specs/` directories

## License

By contributing to Queue-Keeper, you agree that your contributions will be licensed under the same license as the project.
