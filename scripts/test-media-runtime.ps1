param([Parameter(Mandatory = $true)][string]$App)
$ErrorActionPreference = 'Stop'
$start = New-Object Diagnostics.ProcessStartInfo
$start.FileName = (Resolve-Path -LiteralPath $App).Path
$start.Arguments = '--editor-self-test'
$start.UseShellExecute = $false
$start.CreateNoWindow = $true
$start.RedirectStandardOutput = $true
$start.RedirectStandardError = $true
$process = [Diagnostics.Process]::Start($start)
try {
    $output = $process.StandardOutput.ReadToEnd()
    $errorText = $process.StandardError.ReadToEnd()
    $process.WaitForExit()
    if ($process.ExitCode -ne 0 -or $output -notmatch 'Native editor inspection, previews, audio mixing, and export passed') {
        throw "Native editor check failed (exit $($process.ExitCode)): $errorText $output"
    }
    Write-Output $output.Trim()
} finally { $process.Dispose() }
