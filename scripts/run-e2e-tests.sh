#!/usr/bin/env bash
# Run E2E tests locally
# This script builds the Docker image and runs the E2E test suite

set -e

IMAGE_NAME="queue-keeper:test"

echo "🚀 Queue-Keeper E2E Test Runner"
echo ""

# Check if Docker is running
echo "📋 Checking Docker..."
if ! docker ps > /dev/null 2>&1; then
    echo "❌ Docker is not running. Please start Docker."
    exit 1
fi
echo "✅ Docker is running"

# Build Docker image
echo ""
echo "🔨 Building Docker image: $IMAGE_NAME..."
docker build -t "$IMAGE_NAME" .
echo "✅ Docker image built successfully"

# Verify image exists
echo ""
echo "🔍 Verifying Docker image..."
if ! docker inspect "$IMAGE_NAME" > /dev/null 2>&1; then
    echo "❌ Docker image not found"
    exit 1
fi
echo "✅ Docker image verified"

# Run integration tests
echo ""
echo "🧪 Running integration tests..."
cargo test --package queue-keeper-integration-tests --verbose
echo "✅ Integration tests passed"

# Run E2E tests
echo ""
echo "🧪 Running E2E tests against Docker container..."
export RUST_BACKTRACE=1
if cargo test --package queue-keeper-e2e-tests --verbose; then
    TEST_RESULT=0
else
    TEST_RESULT=1
fi

# Cleanup any leftover containers
echo ""
echo "🧹 Cleaning up containers..."
CONTAINERS=$(docker ps -a --filter "ancestor=$IMAGE_NAME" --format "{{.ID}}")
if [ -n "$CONTAINERS" ]; then
    echo "$CONTAINERS" | xargs docker stop > /dev/null 2>&1 || true
    echo "$CONTAINERS" | xargs docker rm > /dev/null 2>&1 || true
    COUNT=$(echo "$CONTAINERS" | wc -l)
    echo "✅ Cleaned up $COUNT container(s)"
else
    echo "✅ No containers to clean up"
fi

# Report results
echo ""
if [ $TEST_RESULT -eq 0 ]; then
    echo "✅ All E2E tests passed!"
    exit 0
else
    echo "❌ E2E tests failed"
    echo ""
    echo "💡 Tips for debugging:"
    echo "  - Check container logs: docker logs <container_id>"
    echo "  - Run a single test: cargo test --package queue-keeper-e2e-tests test_name"
    echo "  - Start container manually: docker run -p 8080:8080 $IMAGE_NAME"
    exit 1
fi
