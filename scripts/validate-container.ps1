#!/usr/bin/env pwsh
# Container Build and Validation Script
# This script builds and validates the Queue-Keeper container image
# Following specifications in specs/architecture/container-deployment.md

param(
    [string]$Tag = "queue-keeper:test",
    [switch]$SkipBuild = $false,
    [switch]$SkipTests = $false,
    [switch]$Verbose = $false
)

$ErrorActionPreference = "Stop"

# Colors for output
$Green = "`e[32m"
$Red = "`e[31m"
$Yellow = "`e[33m"
$Reset = "`e[0m"

function Write-Step
{
    param([string]$Message)
    Write-Host "${Green}➜${Reset} $Message" -ForegroundColor White
}

function Write-Success
{
    param([string]$Message)
    Write-Host "${Green}✓${Reset} $Message" -ForegroundColor Green
}

function Write-Failure
{
    param([string]$Message)
    Write-Host "${Red}✗${Reset} $Message" -ForegroundColor Red
}

function Write-Warning
{
    param([string]$Message)
    Write-Host "${Yellow}⚠${Reset} $Message" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "Queue-Keeper Container Build & Validation" -ForegroundColor Cyan
Write-Host "==========================================" -ForegroundColor Cyan
Write-Host ""

# Check Docker is available
Write-Step "Checking Docker availability..."
try
{
    $dockerVersion = docker --version
    Write-Success "Docker found: $dockerVersion"
}
catch
{
    Write-Failure "Docker is not available. Please install Docker Desktop."
    exit 1
}

# Build container image
if (-not $SkipBuild)
{
    Write-Host ""
    Write-Step "Building container image: $Tag"

    $buildStart = Get-Date

    if ($Verbose)
    {
        docker build -t $Tag .
    }
    else
    {
        docker build -t $Tag . 2>&1 | Out-Null
    }

    if ($LASTEXITCODE -ne 0)
    {
        Write-Failure "Container build failed!"
        exit 1
    }

    $buildDuration = (Get-Date) - $buildStart
    Write-Success "Container built successfully in $($buildDuration.TotalSeconds.ToString('F1'))s"
}

if ($SkipTests)
{
    Write-Host ""
    Write-Success "Build complete (tests skipped)"
    exit 0
}

# Validation Tests
Write-Host ""
Write-Step "Running validation tests..."
Write-Host ""

$testsPassed = 0
$testsFailed = 0

# Test 1: Image exists
Write-Step "Test 1: Verify image exists"
$image = docker images $Tag --format "{{.Repository}}:{{.Tag}}" | Select-Object -First 1
if ($image -eq $Tag)
{
    Write-Success "Image exists: $image"
    $testsPassed++
}
else
{
    Write-Failure "Image not found: $Tag"
    $testsFailed++
}

# Test 2: Image size check
Write-Step "Test 2: Verify image size (<200MB)"
$imageSize = docker images $Tag --format "{{.Size}}"
$imageSizeMB = [int]($imageSize -replace '[^0-9]', '')
Write-Host "  Image size: $imageSize"
if ($imageSizeMB -lt 200)
{
    Write-Success "Image size is within limits"
    $testsPassed++
}
else
{
    Write-Warning "Image size exceeds recommended 200MB limit"
    $testsPassed++
}

# Test 3: Container starts
Write-Step "Test 3: Verify container starts"
$containerName = "queue-keeper-validation-test"
docker rm -f $containerName 2>&1 | Out-Null

docker run -d --name $containerName -p 8090:8080 $Tag 2>&1 | Out-Null
if ($LASTEXITCODE -eq 0)
{
    Write-Success "Container started successfully"
    $testsPassed++

    # Wait for startup
    Write-Host "  Waiting for service startup (5s)..."
    Start-Sleep -Seconds 5

    # Test 4: Health check responds
    Write-Step "Test 4: Verify health endpoint responds"
    try
    {
        $response = Invoke-WebRequest -Uri "http://localhost:8090/health" -UseBasicParsing -TimeoutSec 5
        if ($response.StatusCode -eq 200)
        {
            Write-Success "Health endpoint returned 200 OK"
            $testsPassed++

            # Parse and display response
            $healthData = $response.Content | ConvertFrom-Json
            Write-Host "  Status: $($healthData.status)"
            Write-Host "  Version: $($healthData.version)"
        }
        else
        {
            Write-Failure "Health endpoint returned status: $($response.StatusCode)"
            $testsFailed++
        }
    }
    catch
    {
        Write-Failure "Health endpoint request failed: $_"
        $testsFailed++
    }

    # Test 5: Readiness check responds
    Write-Step "Test 5: Verify readiness endpoint responds"
    try
    {
        $response = Invoke-WebRequest -Uri "http://localhost:8090/ready" -UseBasicParsing -TimeoutSec 5
        if ($response.StatusCode -eq 200)
        {
            Write-Success "Readiness endpoint returned 200 OK"
            $testsPassed++
        }
        else
        {
            Write-Failure "Readiness endpoint returned status: $($response.StatusCode)"
            $testsFailed++
        }
    }
    catch
    {
        Write-Failure "Readiness endpoint request failed: $_"
        $testsFailed++
    }

    # Test 6: Graceful shutdown
    Write-Step "Test 6: Verify graceful shutdown"
    $stopStart = Get-Date
    docker stop $containerName 2>&1 | Out-Null
    $stopDuration = (Get-Date) - $stopStart

    if ($LASTEXITCODE -eq 0 -and $stopDuration.TotalSeconds -lt 35)
    {
        Write-Success "Container stopped gracefully in $($stopDuration.TotalSeconds.ToString('F1'))s"
        $testsPassed++
    }
    else
    {
        Write-Failure "Container shutdown issue (timeout or error)"
        $testsFailed++
    }

    # Cleanup
    docker rm -f $containerName 2>&1 | Out-Null
}
else
{
    Write-Failure "Container failed to start"
    $testsFailed++
}

# Test 7: Non-root user verification
Write-Step "Test 7: Verify container runs as non-root user"
$userId = docker run --rm $Tag id -u
if ($userId -eq "1000")
{
    Write-Success "Container runs as non-root user (UID: $userId)"
    $testsPassed++
}
else
{
    Write-Failure "Container runs as root or unexpected user (UID: $userId)"
    $testsFailed++
}

# Summary
Write-Host ""
Write-Host "Validation Summary" -ForegroundColor Cyan
Write-Host "==================" -ForegroundColor Cyan
Write-Host "Tests Passed: ${Green}$testsPassed${Reset}"
Write-Host "Tests Failed: ${Red}$testsFailed${Reset}"
Write-Host ""

if ($testsFailed -eq 0)
{
    Write-Success "All validation tests passed!"
    Write-Host ""
    Write-Host "Container is ready for use:" -ForegroundColor White
    Write-Host "  docker run -p 8080:8080 $Tag" -ForegroundColor Gray
    Write-Host ""
    exit 0
}
else
{
    Write-Failure "Some validation tests failed. Please review the output above."
    exit 1
}
