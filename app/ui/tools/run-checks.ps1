# Runs every check for the interface: types, unit tests, licenses, the production build, and the
# headless browser check. Everything is pinned to the efficiency cores (CPUs 16 to 31), because the
# performance cores of some processors crash long-running programs.
#
#   powershell -File tools\run-checks.ps1
#   powershell -File tools\run-checks.ps1 -SkipBrowser
#
# The browser check needs Playwright, which is NOT installed in this folder. Point PLAYWRIGHT_DIR at
# an existing install; by default this looks in a playwright-tools folder in your home folder.

param([switch]$SkipBrowser)

$ErrorActionPreference = 'Stop'

try {
    [System.Diagnostics.Process]::GetCurrentProcess().ProcessorAffinity = [IntPtr]4294901760
} catch {
    Write-Host 'Could not pin the program to the efficiency cores; carrying on without it.'
}

if (-not $env:PLAYWRIGHT_DIR) {
    $env:PLAYWRIGHT_DIR = Join-Path $HOME 'playwright-tools\node_modules\playwright'
}

Set-Location (Join-Path $PSScriptRoot '..')

function Step($name, $script) {
    Write-Host ''
    Write-Host "== $name"
    & $script
    if ($LASTEXITCODE -ne 0) {
        Write-Host "FAILED: $name"
        exit 1
    }
}

Step 'Type check' { npm run typecheck }
Step 'Unit tests' { npm test }
Step 'Language catalog' { npm run i18n:test; if ($LASTEXITCODE -eq 0) { npm run i18n:check } }
Step 'License check' { npm run licenses }
Step 'Production build' { npm run build }
if (-not $SkipBrowser) {
    Step 'Browser check (headless)' { npm run browser-check }
}

Write-Host ''
Write-Host 'All checks passed.'
