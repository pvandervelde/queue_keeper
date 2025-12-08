#!/usr/bin/env pwsh
# Run E2E tests locally
# This script builds the Docker image and runs the E2E test suite

$ErrorActionPreference = "Stop"

$IMAGE_NAME = "queue-keeper:test"

Write-Host "🚀 Queue-Keeper E2E Test Runner" -ForegroundColor Cyan
Write-Host ""

# Check if Docker is running
Write-Host "📋 Checking Docker..." -ForegroundColor Yellow
try
{
    docker ps | Out-Null
    Write-Host "✅ Docker is running" -ForegroundColor Green
}
catch
{
    Write-Host "❌ Docker is not running. Please start Docker Desktop." -ForegroundColor Red
    exit 1
}

# Build Docker image
Write-Host ""
Write-Host "🔨 Building Docker image: $IMAGE_NAME..." -ForegroundColor Yellow
docker build -t $IMAGE_NAME .
if ($LASTEXITCODE -ne 0)
{
    Write-Host "❌ Docker build failed" -ForegroundColor Red
    exit 1
}
Write-Host "✅ Docker image built successfully" -ForegroundColor Green

# Verify image exists
Write-Host ""
Write-Host "🔍 Verifying Docker image..." -ForegroundColor Yellow
$imageExists = docker images --format "{{.Repository}}:{{.Tag}}" | Select-String -Pattern "^$IMAGE_NAME$"
if (-not $imageExists)
{
    Write-Host "❌ Docker image not found" -ForegroundColor Red
    exit 1
}
Write-Host "✅ Docker image verified" -ForegroundColor Green

# Run integration tests
Write-Host ""
Write-Host "🧪 Running integration tests..." -ForegroundColor Yellow
cargo test --package queue-keeper-integration-tests --verbose
if ($LASTEXITCODE -ne 0)
{
    Write-Host "❌ Integration tests failed" -ForegroundColor Red
    exit 1
}
Write-Host "✅ Integration tests passed" -ForegroundColor Green

# Run E2E tests
Write-Host ""
Write-Host "🧪 Running E2E tests against Docker container..." -ForegroundColor Yellow
$env:RUST_BACKTRACE = "1"
cargo test --package queue-keeper-e2e-tests --verbose
$testResult = $LASTEXITCODE

# Cleanup any leftover containers
Write-Host ""
Write-Host "🧹 Cleaning up containers..." -ForegroundColor Yellow
$containers = docker ps -a --filter "ancestor=$IMAGE_NAME" --format "{{.ID}}"
if ($containers)
{
    $containers | ForEach-Object {
        docker stop $_ 2>&1 | Out-Null
        docker rm $_ 2>&1 | Out-Null
    }
    Write-Host "✅ Cleaned up $($containers.Count) container(s)" -ForegroundColor Green
}
else
{
    Write-Host "✅ No containers to clean up" -ForegroundColor Green
}

# Report results
Write-Host ""
if ($testResult -eq 0)
{
    Write-Host "✅ All E2E tests passed!" -ForegroundColor Green
    exit 0
}
else
{
    Write-Host "❌ E2E tests failed" -ForegroundColor Red
    Write-Host ""
    Write-Host "💡 Tips for debugging:" -ForegroundColor Yellow
    Write-Host "  - Check container logs: docker logs <container_id>" -ForegroundColor Gray
    Write-Host "  - Run a single test: cargo test --package queue-keeper-e2e-tests test_name" -ForegroundColor Gray
    Write-Host "  - Start container manually: docker run -p 8080:8080 $IMAGE_NAME" -ForegroundColor Gray
    exit 1
}
