#Requires -Version 5.1
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Write-Step([string]$msg) { Write-Host "`n==> $msg" -ForegroundColor Cyan }
function Test-Cmd([string]$cmd)   { $null -ne (Get-Command $cmd -ErrorAction SilentlyContinue) }

function Refresh-Path {
    $machine = [System.Environment]::GetEnvironmentVariable("PATH", [System.EnvironmentVariableTarget]::Machine)
    $user    = [System.Environment]::GetEnvironmentVariable("PATH", [System.EnvironmentVariableTarget]::User)
    $env:PATH = "$machine;$user"
}

# ── 1. Graphviz ──────────────────────────────────────────────────────────────

Write-Step "Checking Graphviz (dot)"

if (Test-Cmd "dot") {
    Write-Host "  dot already on PATH -- skipping." -ForegroundColor Green
} else {
    if (Test-Cmd "winget") {
        Write-Host "  Installing Graphviz via winget..."
        winget install --id Graphviz.Graphviz --accept-package-agreements --accept-source-agreements --silent
    } elseif (Test-Cmd "choco") {
        $isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
            [Security.Principal.WindowsBuiltInRole]::Administrator)
        if ($isAdmin) {
            Write-Host "  Installing Graphviz via Chocolatey..."
            choco install graphviz -y
        } else {
            Write-Warning "  choco requires admin. Run as Administrator or install Graphviz from https://graphviz.org/download/"
        }
    } else {
        Write-Warning "  winget/choco not found. Install Graphviz from https://graphviz.org/download/"
    }
    Refresh-Path
    if (Test-Cmd "dot") {
        Write-Host "  dot installed." -ForegroundColor Green
    } else {
        Write-Warning "  dot not found after install -- re-open terminal and retry."
    }
}

# ── 2. PlantUML ──────────────────────────────────────────────────────────────

Write-Step "Checking PlantUML"

if (Test-Cmd "plantuml") {
    Write-Host "  plantuml already on PATH -- skipping." -ForegroundColor Green
} else {
    $installed = $false

    if (Test-Cmd "winget") {
        Write-Host "  Trying winget install PlantUML..."
        winget install --id PlantUML.PlantUML --accept-package-agreements --accept-source-agreements --silent 2>&1 | Out-Null
        Refresh-Path
        $installed = Test-Cmd "plantuml"
    }

    if (-not $installed -and (Test-Cmd "choco")) {
        $isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
            [Security.Principal.WindowsBuiltInRole]::Administrator)
        if ($isAdmin) {
            Write-Host "  Trying Chocolatey install plantuml..."
            choco install plantuml -y
            Refresh-Path
            $installed = Test-Cmd "plantuml"
        } else {
            Write-Host "  Skipping choco (requires admin) -- will use jar fallback."
        }
    }

    if (-not $installed) {
        Write-Host "  Falling back: downloading plantuml.jar..."

        if (-not (Test-Cmd "java")) {
            Write-Warning "  Java not found. PlantUML requires Java >= 8. See https://adoptium.net"
        }

        $jarDir  = Join-Path $env:USERPROFILE ".local\bin"
        $jarPath = Join-Path $jarDir "plantuml.jar"
        $cmdPath = Join-Path $jarDir "plantuml.cmd"

        New-Item -ItemType Directory -Force -Path $jarDir | Out-Null

        $jarUrl = "https://github.com/plantuml/plantuml/releases/latest/download/plantuml.jar"
        Write-Host "  Downloading $jarUrl ..."
        Invoke-WebRequest -Uri $jarUrl -OutFile $jarPath -UseBasicParsing

        # Write the wrapper without embedded double-quotes confusion
        $cmdLines = '@echo off', ('java -jar "%~dp0plantuml.jar" %*')
        [System.IO.File]::WriteAllLines($cmdPath, $cmdLines, [System.Text.Encoding]::ASCII)

        $userPath = [System.Environment]::GetEnvironmentVariable("PATH", [System.EnvironmentVariableTarget]::User)
        if ($userPath -notlike "*$jarDir*") {
            [System.Environment]::SetEnvironmentVariable(
                "PATH",
                "$userPath;$jarDir",
                [System.EnvironmentVariableTarget]::User)
            $env:PATH += ";$jarDir"
            Write-Host "  Added $jarDir to user PATH (re-open terminal to take effect)." -ForegroundColor Yellow
        }

        $installed = Test-Cmd "plantuml"
        if ($installed) {
            Write-Host "  plantuml.cmd created at $cmdPath" -ForegroundColor Green
        } else {
            Write-Warning "  plantuml not on PATH yet -- open a new terminal."
        }
    }
}

# ── 3. code-mole ─────────────────────────────────────────────────────────────

Write-Step "Building and installing code-mole"

# Ensure %USERPROFILE%\.cargo\bin is in PATH (rustup installs there)
$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
if ($env:PATH -notlike "*$cargoBin*") {
    $env:PATH += ";$cargoBin"
}

if (-not (Test-Cmd "cargo")) {
    Write-Host "  Cargo not found. Installing Rust via rustup..."
    $rustupUrl = "https://win.rustup.rs/x86_64"
    $rustupExe = Join-Path $env:TEMP "rustup-init.exe"
    Invoke-WebRequest -Uri $rustupUrl -OutFile $rustupExe -UseBasicParsing
    & $rustupExe -y --no-modify-path
    Remove-Item $rustupExe -Force
    # Reload PATH so cargo is visible in this session
    $env:PATH += ";$cargoBin"
    if (-not (Test-Cmd "cargo")) {
        Write-Error "Rust installation failed. Install manually from https://rustup.rs"
    }
    Write-Host "  Rust installed." -ForegroundColor Green
} else {
    Write-Host "  cargo found: $(cargo --version)" -ForegroundColor Green
}

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Push-Location $scriptDir
try {
    cargo install --path . --locked
    Write-Host "  code-mole installed." -ForegroundColor Green
} finally {
    Pop-Location
}

Write-Host ""
Write-Host "Done. Run: code-mole --help" -ForegroundColor Green
