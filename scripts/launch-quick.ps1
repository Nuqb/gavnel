# Open the latest completed build without waiting for Cargo or its launcher lock.
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
try {
    $ready = Join-Path $projectRoot 'target\launcher\ready.txt'
    if (-not (Test-Path -LiteralPath $ready)) {
        throw 'No completed build is ready. Run the original Gavnel shortcut once, then use Quick Launch.'
    }
    $hash = [System.IO.File]::ReadAllText($ready).Trim()
    if ($hash -notmatch '^[A-Fa-f0-9]{64}$') { throw 'The completed build reference is invalid. Run the original Gavnel shortcut to repair it.' }
    $app = Join-Path $projectRoot "target\launcher\runs\$hash\Gavnel.exe"
    if (-not (Test-Path -LiteralPath $app)) { throw 'The completed build is missing. Run the original Gavnel shortcut to rebuild it.' }
    # An older instance would absorb the show request and hide the updated build.
    # Use the normal launcher to request a graceful exit and wait for recordings to save.
    $older = @(Get-Process -ErrorAction SilentlyContinue | Where-Object {
        $_.ProcessName -like '*gavnel*' -and $_.Path -and
        $_.Path.StartsWith($projectRoot + '\', [StringComparison]::OrdinalIgnoreCase) -and
        $_.Path -ne $app
    })
    if ($older.Count -gt 0) {
        & (Join-Path $PSScriptRoot 'launch.ps1')
        exit $LASTEXITCODE
    }
    # A second instance asks the current build to show itself.
    $launched = Start-Process -FilePath $app -WorkingDirectory $projectRoot -WindowStyle Normal -PassThru
    if ($launched.WaitForExit(1500) -and $launched.ExitCode -ne 0) {
        throw "Gavnel exited during startup (code $($launched.ExitCode))."
    }
} catch {
    Add-Type -AssemblyName System.Windows.Forms
    [void][System.Windows.Forms.MessageBox]::Show($_.Exception.Message, 'Gavnel Quick Launch')
    exit 1
}
