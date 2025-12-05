# Container Usage Guide

This document provides instructions for building, running, and testing the Queue-Keeper service container.

## Overview

The Queue-Keeper service is containerized using a multi-stage Docker build that:
- Optimizes final image size (target: <200MB)
- Runs as non-root user for security
- Includes health check integration
- Supports configuration via environment variables

## Building the Container

### Local Build

Build the container image locally:

```bash
docker build -t queue-keeper:latest .
```

Build with a specific version tag:

```bash
docker build -t queue-keeper:v0.1.0 .
```

### Build Options

The build process uses Rust's release profile for optimal performance:
- Full optimization (opt-level = 3)
- Link-time optimization (LTO)
- Stripped debug symbols

Build time: Approximately 5-10 minutes on first build (dependencies cached for subsequent builds)

## Running the Container

### Basic Usage

Run the container with default settings:

```bash
docker run -p 8080:8080 queue-keeper:latest
```

### With Environment Variables

Configure the service using environment variables:

```bash
docker run -p 8080:8080 \
  -e QUEUE_KEEPER_LOG_LEVEL=debug \
  -e QUEUE_KEEPER_PORT=8080 \
  -e QUEUE_KEEPER_HOST=0.0.0.0 \
  queue-keeper:latest
```

### With Configuration File

Mount a configuration file into the container:

```bash
docker run -p 8080:8080 \
  -v $(pwd)/config:/home/queuekeeper/config:ro \
  queue-keeper:latest
```

### Background Mode

Run as a daemon:

```bash
docker run -d \
  --name queue-keeper \
  -p 8080:8080 \
  queue-keeper:latest
```

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `QUEUE_KEEPER_LOG_LEVEL` | `info` | Logging level (trace, debug, info, warn, error) |
| `QUEUE_KEEPER_PORT` | `8080` | HTTP server port |
| `QUEUE_KEEPER_HOST` | `0.0.0.0` | HTTP server bind address |
| `RUST_BACKTRACE` | `1` | Enable Rust backtraces for errors |

### Additional Configuration

For production deployments, additional environment variables may be required:
- GitHub webhook secrets (via Key Vault integration)
- Queue connection strings (Azure Service Bus or AWS SQS)
- Blob storage configuration

See `specs/architecture/container-deployment.md` for complete configuration details.

## Health Checks

### Container Health Check

The container includes an integrated health check that:
- Runs every 30 seconds
- Times out after 3 seconds
- Allows 5 seconds startup grace period
- Marks unhealthy after 3 consecutive failures

Check container health status:

```bash
docker inspect --format='{{.State.Health.Status}}' queue-keeper
```

### Manual Health Check

Test the health endpoint directly:

```bash
# Basic health check
curl http://localhost:8080/health

# Deep health check (includes dependencies)
curl http://localhost:8080/health/deep

# Readiness check (for Kubernetes)
curl http://localhost:8080/ready
```

Expected response:
```json
{
  "status": "healthy",
  "timestamp": "2025-12-06T10:30:00.000Z",
  "checks": {
    "service": {
      "healthy": true,
      "message": "Service is running",
      "duration_ms": 0
    }
  },
  "version": "0.1.0"
}
```

## Testing the Container

### Container Build Test

Verify the container builds successfully:

```bash
docker build -t queue-keeper:test .
echo "Build exit code: $?"
```

### Image Size Check

Verify the image meets size requirements (<200MB):

```bash
docker images queue-keeper:test --format "{{.Size}}"
```

### Security Scan

Scan for vulnerabilities using trivy:

```bash
# Install trivy if not already installed
# See https://aquasecurity.github.io/trivy/latest/getting-started/installation/

trivy image queue-keeper:test
```

### Run Test

Start the container and verify it responds:

```bash
# Start container
docker run -d --name queue-keeper-test -p 8080:8080 queue-keeper:test

# Wait for startup
sleep 5

# Test health endpoint
curl -f http://localhost:8080/health

# Stop and remove
docker stop queue-keeper-test
docker rm queue-keeper-test
```

### Graceful Shutdown Test

Verify the container stops gracefully:

```bash
# Start container
docker run -d --name queue-keeper-test -p 8080:8080 queue-keeper:test

# Stop with timeout observation
time docker stop queue-keeper-test

# Should complete within configured shutdown timeout (default 30s)
# Actual time will be shorter if no active requests
```

## Troubleshooting

### Container Won't Start

Check container logs:

```bash
docker logs queue-keeper
```

Common issues:
- Port 8080 already in use (change with `-p` flag)
- Missing configuration or environment variables
- Insufficient permissions

### Health Check Failing

View health check logs:

```bash
docker inspect --format='{{json .State.Health}}' queue-keeper | jq
```

Common causes:
- Application startup taking longer than 5 seconds
- Health endpoint returning non-200 status
- Network connectivity issues

### Performance Issues

Check container resource usage:

```bash
docker stats queue-keeper
```

Expected resource usage:
- CPU: <60% average under normal load
- Memory: <256MB baseline, <512MB maximum

## Production Deployment

For production deployments to Azure Container Apps, see:
- `specs/architecture/container-deployment.md` - Deployment architecture
- `specs/operations/deployment.md` - Deployment procedures

Key considerations:
- Use multi-replica deployment (minimum 2)
- Configure resource limits (250m CPU, 256Mi memory baseline)
- Enable auto-scaling based on load
- Use managed identity for Azure service authentication
- Implement blue-green deployment strategy

## Security Best Practices

The container follows security best practices:

1. **Non-root user**: Application runs as `queuekeeper` user (UID 1000)
2. **Minimal base image**: Debian bullseye-slim with only required packages
3. **No secrets in image**: Configuration via environment variables only
4. **Health checks**: Automatic detection of unhealthy containers
5. **Regular scans**: Use trivy for vulnerability scanning

For production:
- Scan images regularly for vulnerabilities
- Update base images and dependencies
- Rotate secrets and credentials
- Monitor security advisories

## Additional Resources

- [Docker Documentation](https://docs.docker.com/)
- [Container Best Practices](https://docs.docker.com/develop/dev-best-practices/)
- [Security Scanning with Trivy](https://aquasecurity.github.io/trivy/)
- Queue-Keeper Specifications in `specs/` directory
