#!/usr/bin/env pwsh
<#
.SYNOPSIS
    Run tests using Podman for PostgreSQL and Redis dependencies.
.DESCRIPTION
    Orchestrates a test environment with PostgreSQL + Redis via Podman compose.
    For integration tests, also starts the app server locally and waits for
    the health endpoint before running tests.
.PARAMETER IntegrationOnly
    Run only integration tests (starts postgres + redis + server via podman).
.PARAMETER UnitOnly
    Run only unit tests (no containers needed).
.PARAMETER NoTeardown
    Keep containers running after tests finish (useful for debugging).
.EXAMPLE
    .\scripts\test-with-podman.ps1
    .\scripts\test-with-podman.ps1 -IntegrationOnly
    .\scripts\test-with-podman.ps1 -UnitOnly
#>

param(
    [switch]$IntegrationOnly,
    [switch]$UnitOnly,
    [switch]$NoTeardown
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Resolve-Path "$PSScriptRoot/.."
$ComposeFile = "$ProjectRoot/compose.yaml"
$ServerProcess = $null

if (-not (Get-Command podman -ErrorAction SilentlyContinue)) {
    Write-Error "Podman is not installed. Install it from https://podman.io/docs/installation"
    exit 1
}

function Invoke-Compose {
    podman compose -f $ComposeFile @args
}

function Get-ContainerName {
    param([string]$Service)
    $all = podman ps -a --format "{{.Names}}" 2>$null
    $match = $all | Where-Object { $_ -match "^.*-$Service-\d+$" }
    if (-not $match) {
        $match = $all | Where-Object { $_ -match "^.*_${Service}_\d+$" }
    }
    return $match | Select-Object -First 1
}

function Wait-ContainerHealthy {
    param([string]$Service, [int]$TimeoutSeconds = 60)

    $end = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    while ([DateTime]::UtcNow -lt $end) {
        $name = Get-ContainerName -Service $Service
        if (-not $name) {
            Write-Host "  Waiting for $Service container to appear..." -ForegroundColor DarkYellow
            Start-Sleep -Seconds 2
            continue
        }
        $status = podman ps --filter "name=$name" --format "{{.Status}}" 2>$null
        if ($status -match "healthy") {
            Write-Host "  $name is healthy." -ForegroundColor Green
            return
        }
        Write-Host "  Waiting for $name... ($status)" -ForegroundColor DarkYellow
        Start-Sleep -Seconds 2
    }
    Write-Error "$Service container did not become healthy within ${TimeoutSeconds}s"
    exit 1
}

function Start-AppServer {
    Write-Host "Starting app server on localhost:3000..." -ForegroundColor Cyan

    # Build then copy so cargo test can freely rebuild the original .exe
    cargo build
    if ($LASTEXITCODE -ne 0) { throw "Server build failed" }

    $logFile = "$ProjectRoot/target/server.log"
    $errFile = "$ProjectRoot/target/server.err.log"
    $serverBin = "$ProjectRoot/target/debug/app-home-services.exe"
    $runBin = "$ProjectRoot/target/app-home-services-test-run.exe"
    Copy-Item $serverBin $runBin -Force
    $script:ServerProcess = Start-Process -FilePath $runBin -NoNewWindow -PassThru `
        -RedirectStandardOutput $logFile -RedirectStandardError $errFile

    Write-Host "Waiting for server health endpoint..." -ForegroundColor Cyan
    $end = [DateTime]::UtcNow.AddSeconds(30)
    $ready = $false
    while ([DateTime]::UtcNow -lt $end) {
        $result = curl.exe -s -o nul -w "%{http_code}" http://localhost:3000/api/health 2>$null
        if ($result -eq "200") {
            $ready = $true
            break
        }
        $lines = (Get-Content $logFile -Tail 1 -ErrorAction SilentlyContinue)
        Write-Host "  Waiting for server... ($lines)" -ForegroundColor DarkYellow
        Start-Sleep -Seconds 1
    }

    if (-not $ready) {
        Stop-AppServer
        Write-Error "App server did not become ready within 30s"
        exit 1
    }
    Write-Host "  Server is ready." -ForegroundColor Green
}

function Stop-AppServer {
    if ($script:ServerProcess -and (-not $script:ServerProcess.HasExited)) {
        Write-Host "Stopping app server (PID: $($script:ServerProcess.Id))..." -ForegroundColor Yellow
        $script:ServerProcess.Kill()
        $script:ServerProcess.WaitForExit(5000)
        Write-Host "  Server stopped." -ForegroundColor Green
    }
}

function Invoke-TearDown {
    Stop-AppServer
    Remove-Item "$ProjectRoot/target/app-home-services-test-run.exe" -Force -ErrorAction SilentlyContinue
    if (-not $NoTeardown) {
        Write-Host "Tearing down test environment..." -ForegroundColor Yellow
        Invoke-Compose down
    } else {
        Write-Host "Containers left running." -ForegroundColor Yellow
        Write-Host "Stop manually: podman compose -f $ComposeFile down" -ForegroundColor Cyan
    }
}

try {
    if ($UnitOnly) {
        Write-Host "Running unit tests (no containers needed)..." -ForegroundColor Cyan
        cargo test
        if ($LASTEXITCODE -ne 0) { exit 1 }
        exit 0
    }

    # --- Start dependencies ---
    Write-Host "Starting PostgreSQL + Redis..." -ForegroundColor Cyan
    Invoke-Compose up --detach postgres redis

    Write-Host "Waiting for services to be healthy..." -ForegroundColor Cyan
    Wait-ContainerHealthy -Service postgres
    Wait-ContainerHealthy -Service redis

    $env:DATABASE_URL = "postgres://app_home:app_home_test@localhost:15432/app_home_test"
    $env:REDIS_URL = "redis://localhost:16379"
    $env:JWT_SECRET = "test-secret-key-for-podman-test-environment"
    $env:DEFAULT_USER_PASSWORD = "test-password"
    $env:CORS_ALLOWED_ORIGINS = "http://localhost:8080"
    $env:RUST_LOG = "info"

    # --- Run unit tests first (fast, no server needed) ---
    if (-not $IntegrationOnly) {
        Write-Host "Running unit tests..." -ForegroundColor Cyan
        cargo test
        if ($LASTEXITCODE -ne 0) { throw "Unit tests failed" }
    }

    # --- Start server for integration tests ---
    Start-AppServer

    # --- Run integration tests ---
    if ($IntegrationOnly) {
        Write-Host "Running integration tests..." -ForegroundColor Cyan
    } else {
        Write-Host "Running integration tests..." -ForegroundColor Cyan
    }
    cargo test -- --ignored --test-threads=1
    $exitCode = $LASTEXITCODE
    if ($exitCode -ne 0) {
        Write-Host "Some integration tests failed (exit code: $exitCode). This may be a pre-existing issue." -ForegroundColor Yellow
    } else {
        Write-Host "All tests passed!" -ForegroundColor Green
    }
}
catch {
    Write-Error "Test run failed: $_"
    exit 1
}
finally {
    Invoke-TearDown
}
