param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$')]
    [string]$Version,
    [switch]$Resume
)
$ErrorActionPreference = 'Stop'
if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
    throw 'Install GitHub CLI (gh), then run gh auth login before releasing.'
}
Push-Location (Split-Path -Parent $PSScriptRoot)
try {
    $arguments = @('scripts/release.py', 'ship', '--tag', "v$Version")
    if ($Resume) { $arguments += '--resume' }
    & python @arguments
    if ($LASTEXITCODE -ne 0) { throw 'Release did not complete. See the error above and docs/releases.md.' }
} finally { Pop-Location }
