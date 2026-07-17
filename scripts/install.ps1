#Requires -Version 5.1
# install.ps1 — installs optional dependencies only (no Rust build)
#   • Graphviz (optional SVG renderer)
#   • Java 17+ JDK  (required for --lang java)
#   • Maven 3.6+    (required to build the Java AST parser)
#   • Builds the Java parser fat-jar  (parsers\java\java-parser.jar)
#
# Use install-all.ps1 to also build and install the codemole binary.
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Write-Step([string]$msg) { Write-Host "`n==> $msg" -ForegroundColor Cyan }
function Write-Ok([string]$msg)   { Write-Host "  [ok] $msg" -ForegroundColor Green }
function Test-Cmd([string]$cmd)   { $null -ne (Get-Command $cmd -ErrorAction SilentlyContinue) }
function Test-Admin               {
    ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
        [Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Refresh-Path {
    $machine = [System.Environment]::GetEnvironmentVariable("PATH", [System.EnvironmentVariableTarget]::Machine)
    $user    = [System.Environment]::GetEnvironmentVariable("PATH", [System.EnvironmentVariableTarget]::User)
    $env:PATH = "$machine;$user"
}

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot  = Split-Path -Parent $scriptDir

# ── 1. Graphviz ──────────────────────────────────────────────────────────────

Write-Step "Checking Graphviz (dot)"

if (Test-Cmd "dot") {
    Write-Ok "dot already on PATH -- skipping."
} else {
    if (Test-Cmd "winget") {
        winget install --id Graphviz.Graphviz --accept-package-agreements --accept-source-agreements --silent
    } elseif ((Test-Cmd "choco") -and (Test-Admin)) {
        choco install graphviz -y
    } else {
        Write-Warning "  Install Graphviz from https://graphviz.org/download/ (optional)"
    }
    Refresh-Path
    if (Test-Cmd "dot") { Write-Ok "dot installed." }
    else { Write-Warning "dot not found -- re-open terminal or install manually (optional)." }
}

# ── 2. Java 17+ ──────────────────────────────────────────────────────────────

Write-Step "Checking Java 17+ (required for --lang java)"

function Install-Java {
    if (Test-Cmd "winget") {
        Write-Host "  Installing Microsoft OpenJDK 17 via winget..."
        winget install --id Microsoft.OpenJDK.17 --accept-package-agreements --accept-source-agreements --silent
    } elseif ((Test-Cmd "choco") -and (Test-Admin)) {
        Write-Host "  Installing OpenJDK 17 via Chocolatey..."
        choco install microsoft-openjdk17 -y
    } else {
        Write-Warning "  Install Java 17+ from https://adoptium.net and re-run."
        return $false
    }
    Refresh-Path
    return (Test-Cmd "java")
}

$javaOk = $false
if (Test-Cmd "java") {
    $javaVer = & { $ErrorActionPreference = 'Continue'; java -version 2>&1 | ForEach-Object { "$_" } | Select-String 'version "(\d+)' } | Select-Object -First 1
    $javaVer = $javaVer.Matches[0].Groups[1].Value
    if ([int]$javaVer -ge 17) {
        Write-Ok "java $javaVer -- OK"
        $javaOk = $true
    } else {
        Write-Warning "Java $javaVer found, need 17+. Installing..."
        $javaOk = Install-Java
    }
} else {
    Write-Host "  Java not found. Installing..."
    $javaOk = Install-Java
}

# ── 3. Maven 3.6+ ────────────────────────────────────────────────────────────

Write-Step "Checking Maven (required to build java-parser.jar)"

function Install-Maven {
    if (Test-Cmd "winget") {
        winget install --id Apache.Maven --accept-package-agreements --accept-source-agreements --silent 2>&1 | Out-Null
        Refresh-Path
    } elseif ((Test-Cmd "choco") -and (Test-Admin)) {
        choco install maven -y
        Refresh-Path
    } else {
        $mvnVer  = "3.9.7"
        $mvnUrl  = "https://archive.apache.org/dist/maven/maven-3/$mvnVer/binaries/apache-maven-$mvnVer-bin.zip"
        $mvnZip  = Join-Path $env:TEMP "apache-maven-$mvnVer-bin.zip"
        $mvnDest = Join-Path $env:USERPROFILE ".local\share\apache-maven-$mvnVer"
        if (-not (Test-Path $mvnDest)) {
            Write-Host "  Downloading Maven $mvnVer..."
            Invoke-WebRequest -Uri $mvnUrl -OutFile $mvnZip -UseBasicParsing
            Expand-Archive -Path $mvnZip -DestinationPath (Split-Path $mvnDest) -Force
            Remove-Item $mvnZip -Force
        }
        $mvnBin = Join-Path $mvnDest "bin"
        $env:PATH += ";$mvnBin"
        $userPath = [System.Environment]::GetEnvironmentVariable("PATH", [System.EnvironmentVariableTarget]::User)
        if ($userPath -notlike "*$mvnBin*") {
            [System.Environment]::SetEnvironmentVariable("PATH", "$userPath;$mvnBin", [System.EnvironmentVariableTarget]::User)
        }
    }
    return (Test-Cmd "mvn")
}

$mvnOk = $false
if (Test-Cmd "mvn") {
    Write-Ok "mvn found: $(mvn --version 2>&1 | Select-Object -First 1)"
    $mvnOk = $true
} else {
    Write-Host "  Maven not found. Installing..."
    $mvnOk = Install-Maven
    if ($mvnOk) { Write-Ok "mvn installed." }
    else { Write-Warning "  Maven not found after install. Build java-parser.jar manually." }
}

# ── 4. Build java-parser.jar ─────────────────────────────────────────────────

Write-Step "Building java-parser.jar"

$javaParserDir = Join-Path $repoRoot "parsers\java"
$jarPath       = Join-Path $javaParserDir "target\java-parser.jar"

if ($javaOk -and $mvnOk) {
    Write-Host "  Running: mvn -q package -f $javaParserDir\pom.xml ..."
    Push-Location $javaParserDir
    try {
        mvn -q package -DskipTests
        if (Test-Path $jarPath) {
            Write-Ok "java-parser.jar built at $jarPath"
            $releaseDir = Join-Path $repoRoot "target\release"
            $null = New-Item -ItemType Directory -Force -Path $releaseDir
            Copy-Item $jarPath (Join-Path $releaseDir "java-parser.jar") -Force
            Write-Ok "java-parser.jar copied to $releaseDir"
        } else {
            Write-Warning "Build finished but jar not found at $jarPath"
        }
    } finally {
        Pop-Location
    }
} else {
    Write-Warning "Skipping jar build (Java or Maven unavailable)."
    Write-Warning "Run manually: cd parsers\java ; mvn package"
}

# ── Done ──────────────────────────────────────────────────────────────────────

Write-Host ""
Write-Host "Done. Run: codemole --help" -ForegroundColor Green
Write-Host "If you have troubles executing codemole ensure codemole.exe is in your path environment variable."