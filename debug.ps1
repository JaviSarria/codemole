#!/usr/bin/env powershell

# Debug script for codemole call extraction
# Usage: .\debug.ps1 -JavaFile "path/to/Service.java" -MethodName "crudTicket"

param(
    [Parameter(Mandatory=$true)]
    [string]$JavaFile,
    
    [Parameter(Mandatory=$true)]
    [string]$MethodName,
    
    [string]$CodemolePath = ".\target\release\examples\debug_calls.exe"
)

if (-not (Test-Path $CodemolePath)) {
    Write-Host "Error: debug_calls tool not found at $CodemolePath" -ForegroundColor Red
    Write-Host "Build it with: cargo build --example debug_calls --release" -ForegroundColor Yellow
    exit 1
}

if (-not (Test-Path $JavaFile)) {
    Write-Host "Error: Java file not found: $JavaFile" -ForegroundColor Red
    exit 1
}

Write-Host "=" * 80 -ForegroundColor Cyan
Write-Host "CODEMOLE DEBUG - Call Extraction Analysis" -ForegroundColor Cyan
Write-Host "=" * 80 -ForegroundColor Cyan
Write-Host ""

& $CodemolePath $JavaFile $MethodName

Write-Host ""
Write-Host "=" * 80 -ForegroundColor Green
Write-Host "DIAGNOSTIC TIPS:" -ForegroundColor Green
Write-Host "=" * 80 -ForegroundColor Green
Write-Host ""
Write-Host "1. Check 'Kept for traversal:' section"
Write-Host "   - Are all your methods listed?"
Write-Host "   - If not, they may be filtered by skip-symbols.db"
Write-Host ""
Write-Host "2. If methods are missing from 'Kept' section:"
Write-Host "   a) They might be in a different class - check imports"
Write-Host "   b) They might have names matching skip-symbols (check Filtered list)"
Write-Host "   c) They might not be in any .java file being scanned"
Write-Host ""
Write-Host "3. Next step: Run codemole with verbose output"
Write-Host "   codemole --lang java --path <root> --endpoint <name> --output <dir>"
Write-Host "   Check the 'Call graph: X nodes, Y edges' line"
Write-Host "   If Y is 0, no calls are being followed at all"
Write-Host ""
