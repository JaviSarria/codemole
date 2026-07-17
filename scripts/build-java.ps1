#Requires -Version 5.1
# build-java.ps1 — builds the Java parser fat-jar via Maven.
# Kept for manual use; the Makefile uses build-java.bat on Windows.
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location (Join-Path $repoRoot "parsers\java")
mvn -q package -DskipTests
