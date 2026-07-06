<#
.SYNOPSIS
    Starts a local PostgreSQL container via Podman for app-home-services.

.DESCRIPTION
    - Creates (or restarts) a named Podman container running Postgres.
    - Uses a named volume so data survives container restarts.
    - Publishes on the standard port 5432 by default (this is for local dev,
      not the test environment in compose.yaml, which uses 15432 to avoid
      clashing with exactly this kind of local Postgres).
    - Waits until Postgres reports healthy before returning.
    - Prints the DATABASE_URL to paste into your .env.

.PARAMETER PgUser
    Postgres username to create. Required.

.PARAMETER PgPassword
    Postgres password for PgUser. Required. Pass it single-quoted on the
    command line if it contains $, !, or other shell-special characters, e.g.
    -PgPassword 'K7$mQx2pR!vL9wZt'

.PARAMETER PgDatabase
    Database name to create. Required.

.PARAMETER PgPort
    Host port to publish Postgres on. Defaults to 5432.

.PARAMETER ContainerName
    Podman container name. Defaults to "apphome-postgres-dev".

.PARAMETER Reset
    Removes the existing container AND its data volume first, then creates
    everything from scratch. Use this if you want a completely clean database.

.EXAMPLE
    .\run-postgres-dev.ps1 -PgUser manuel -PgPassword 'K7$mQx2pR!vL9wZt' -PgDatabase apphome

.EXAMPLE
    .\run-postgres-dev.ps1 -PgUser manuel -PgPassword 'K7$mQx2pR!vL9wZt' -PgDatabase apphome -Reset
#>

[Diagnostics.CodeAnalysis.SuppressMessageAttribute(
    'PSAvoidUsingPlainTextForPassword', '',
    Justification = 'Dev-only script; password must be passed as plaintext to Podman env vars')]
param(
    [Parameter(Mandatory = $true)]
    [string]$PgUser,

    [Parameter(Mandatory = $true)]
    [string]$PgPassword,

    [Parameter(Mandatory = $true)]
    [string]$PgDatabase,

    [int]$PgPort = 5432,

    [string]$ContainerName = "apphome-postgres-dev",

    [switch]$Reset
)

$ErrorActionPreference = "Stop"

$VolumeName = "$ContainerName-data"

function Test-Podman {
    if (-not (Get-Command podman -ErrorAction SilentlyContinue)) {
        Write-Error "Podman not found. Install it from https://podman.io/docs/installation"
        exit 1
    }
}

function Remove-Existing {
    $existing = podman ps -a --filter "name=^${ContainerName}$" --format "{{.Names}}"
    if ($existing) {
        Write-Host "Removing existing container '$ContainerName'..."
        podman rm -f $ContainerName | Out-Null
    }
    if ($Reset) {
        $volExists = podman volume ls --filter "name=^${VolumeName}$" --format "{{.Name}}"
        if ($volExists) {
            Write-Host "Removing existing volume '$VolumeName' (full reset)..."
            podman volume rm $VolumeName | Out-Null
        }
    }
}

function Start-Postgres {
    Write-Host "Starting Postgres container '$ContainerName' on port $PgPort..."
    podman run -d `
        --name $ContainerName `
        -e POSTGRES_USER=$PgUser `
        -e POSTGRES_PASSWORD=$PgPassword `
        -e POSTGRES_DB=$PgDatabase `
        -p "${PgPort}:5432" `
        -v "${VolumeName}:/var/lib/postgresql/data" `
        docker.io/postgres:17-alpine | Out-Null
}

function Wait-Healthy {
    Write-Host "Waiting for Postgres to become ready..." -NoNewline
    $maxAttempts = 30
    for ($i = 0; $i -lt $maxAttempts; $i++) {
        podman exec $ContainerName pg_isready -U $PgUser 2>$null | Out-Null
        if ($LASTEXITCODE -eq 0) {
            Write-Host " ready!"
            return
        }
        Write-Host "." -NoNewline
        Start-Sleep -Seconds 1
    }
    Write-Host ""
    Write-Error "Postgres did not become ready in time. Check logs with: podman logs $ContainerName"
    exit 1
}

# URL-encodes a string for safe use in the userinfo part of a connection URL
# (covers the characters most likely to show up in a generated password).
function ConvertTo-UrlEncoded {
    param([string]$Value)
    return [uri]::EscapeDataString($Value)
}

Test-Podman
Remove-Existing
Start-Postgres
Wait-Healthy

$encodedPassword = ConvertTo-UrlEncoded $PgPassword

Write-Host ""
Write-Host "Postgres is up. Use this in your .env:"
Write-Host "DATABASE_URL=postgresql://${PgUser}:${encodedPassword}@localhost:${PgPort}/${PgDatabase}"
Write-Host ""
Write-Host "Useful commands:"
Write-Host "  podman logs -f $ContainerName        # view logs"
Write-Host "  podman exec -it $ContainerName psql -U $PgUser -d $PgDatabase   # open a psql shell"
Write-Host "  podman stop $ContainerName            # stop (data persists in the '$VolumeName' volume)"
Write-Host "  .\run-postgres-dev.ps1 -PgUser $PgUser -PgPassword '<password>' -PgDatabase $PgDatabase -Reset   # wipe and start fresh"