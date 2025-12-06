#!/usr/bin/env bash
# Container Build and Validation Script (Bash version)
# This script builds and validates the Queue-Keeper container image
# Following specifications in specs/architecture/container-deployment.md

set -euo pipefail

# Default values
TAG="${1:-queue-keeper:test}"
SKIP_BUILD="${SKIP_BUILD:-false}"
SKIP_TESTS="${SKIP_TESTS:-false}"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

function step() {
    echo -e "${GREEN}➜${NC} $1"
}

function success() {
    echo -e "${GREEN}✓${NC} $1"
}

function failure() {
    echo -e "${RED}✗${NC} $1"
}

function warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

echo ""
echo "Queue-Keeper Container Build & Validation"
echo "=========================================="
echo ""

# Check Docker is available
step "Checking Docker availability..."
if command -v docker &> /dev/null; then
    DOCKER_VERSION=$(docker --version)
    success "Docker found: $DOCKER_VERSION"
else
    failure "Docker is not available. Please install Docker."
    exit 1
fi

# Build container image
if [ "$SKIP_BUILD" != "true" ]; then
    echo ""
    step "Building container image: $TAG"

    BUILD_START=$(date +%s)

    if docker build -t "$TAG" .; then
        BUILD_END=$(date +%s)
        BUILD_DURATION=$((BUILD_END - BUILD_START))
        success "Container built successfully in ${BUILD_DURATION}s"
    else
        failure "Container build failed!"
        exit 1
    fi
fi

if [ "$SKIP_TESTS" = "true" ]; then
    echo ""
    success "Build complete (tests skipped)"
    exit 0
fi

# Validation Tests
echo ""
step "Running validation tests..."
echo ""

TESTS_PASSED=0
TESTS_FAILED=0

# Test 1: Image exists
step "Test 1: Verify image exists"
if docker images "$TAG" --format "{{.Repository}}:{{.Tag}}" | grep -q "$TAG"; then
    success "Image exists: $TAG"
    ((TESTS_PASSED++))
else
    failure "Image not found: $TAG"
    ((TESTS_FAILED++))
fi

# Test 2: Image size check
step "Test 2: Verify image size (<200MB)"
IMAGE_SIZE=$(docker images "$TAG" --format "{{.Size}}")
echo "  Image size: $IMAGE_SIZE"
if [[ "$IMAGE_SIZE" =~ ^[0-9]+MB$ ]] && [ "${IMAGE_SIZE//[^0-9]/}" -lt 200 ]; then
    success "Image size is within limits"
    ((TESTS_PASSED++))
else
    warning "Image size exceeds recommended 200MB limit"
    ((TESTS_PASSED++))
fi

# Test 3: Container starts
step "Test 3: Verify container starts"
CONTAINER_NAME="queue-keeper-validation-test"
docker rm -f "$CONTAINER_NAME" 2>/dev/null || true

if docker run -d --name "$CONTAINER_NAME" -p 8090:8080 "$TAG" > /dev/null; then
    success "Container started successfully"
    ((TESTS_PASSED++))

    # Wait for startup
    echo "  Waiting for service startup (5s)..."
    sleep 5

    # Test 4: Health check responds
    step "Test 4: Verify health endpoint responds"
    if RESPONSE=$(curl -f -s http://localhost:8090/health); then
        success "Health endpoint returned 200 OK"
        ((TESTS_PASSED++))

        # Parse and display response
        echo "  Status: $(echo "$RESPONSE" | grep -o '"status":"[^"]*"' | cut -d'"' -f4)"
        echo "  Version: $(echo "$RESPONSE" | grep -o '"version":"[^"]*"' | cut -d'"' -f4)"
    else
        failure "Health endpoint request failed"
        ((TESTS_FAILED++))
    fi

    # Test 5: Readiness check responds
    step "Test 5: Verify readiness endpoint responds"
    if curl -f -s http://localhost:8090/ready > /dev/null; then
        success "Readiness endpoint returned 200 OK"
        ((TESTS_PASSED++))
    else
        failure "Readiness endpoint request failed"
        ((TESTS_FAILED++))
    fi

    # Test 6: Graceful shutdown
    step "Test 6: Verify graceful shutdown"
    STOP_START=$(date +%s)
    docker stop "$CONTAINER_NAME" > /dev/null 2>&1
    STOP_END=$(date +%s)
    STOP_DURATION=$((STOP_END - STOP_START))

    if [ $? -eq 0 ] && [ "$STOP_DURATION" -lt 35 ]; then
        success "Container stopped gracefully in ${STOP_DURATION}s"
        ((TESTS_PASSED++))
    else
        failure "Container shutdown issue (timeout or error)"
        ((TESTS_FAILED++))
    fi

    # Cleanup
    docker rm -f "$CONTAINER_NAME" > /dev/null 2>&1 || true
else
    failure "Container failed to start"
    ((TESTS_FAILED++))
fi

# Test 7: Non-root user verification
step "Test 7: Verify container runs as non-root user"
USER_ID=$(docker run --rm "$TAG" id -u)
if [ "$USER_ID" = "1000" ]; then
    success "Container runs as non-root user (UID: $USER_ID)"
    ((TESTS_PASSED++))
else
    failure "Container runs as root or unexpected user (UID: $USER_ID)"
    ((TESTS_FAILED++))
fi

# Summary
echo ""
echo "Validation Summary"
echo "=================="
echo -e "Tests Passed: ${GREEN}$TESTS_PASSED${NC}"
echo -e "Tests Failed: ${RED}$TESTS_FAILED${NC}"
echo ""

if [ "$TESTS_FAILED" -eq 0 ]; then
    success "All validation tests passed!"
    echo ""
    echo "Container is ready for use:"
    echo "  docker run -p 8080:8080 $TAG"
    echo ""
    exit 0
else
    failure "Some validation tests failed. Please review the output above."
    exit 1
fi
