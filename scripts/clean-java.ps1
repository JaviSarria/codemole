#Requires -Version 5.1
# clean-java.ps1 — cleans the Java parser Maven build.
# Called by the Makefile on Windows to avoid MSYS2 argument mangling.
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location (Join-Path $repoRoot "parsers\java")
mvn -q clean
