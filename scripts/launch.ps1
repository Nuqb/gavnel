param([switch]$BuildOnly, [switch]$Background, [switch]$OnboardingPreview)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
$projectRoot = Split-Path -Parent $PSScriptRoot
$log = Join-Path $projectRoot 'work\launcher.log'
New-Item -ItemType Directory -Force (Split-Path $log) | Out-Null
$lock = [System.Threading.Mutex]::new($false, 'Local\Gavnel.SourceLauncher')
$locked = $false
try {
    if (-not $BuildOnly -and -not $Background) {
        Write-Host 'Starting Gavnel. Checking the latest source; a rebuild can take a few minutes.'
        Write-Host "Progress and errors are saved to $log"
    }
    # A desktop click during a BuildOnly run must still launch once it finishes.
    try { $locked = $lock.WaitOne(120000) } catch [System.Threading.AbandonedMutexException] { $locked = $true }
    if (-not $locked) { throw 'Gavnel is still building or finishing an earlier launch. Please try again shortly.' }
    "`r`n[$(Get-Date -Format o)] Launcher started (BuildOnly=$BuildOnly, Background=$Background)" | Add-Content -LiteralPath $log
    Push-Location $projectRoot
    try {
        # Cargo checks its dependency graph, including assets, on every launch.
        # Use a separate output folder so a running build never locks the compiler output.
        $ErrorActionPreference = 'Continue'
        & cargo build --release --locked --target-dir target/launcher 2>&1 | ForEach-Object {
            if (-not $BuildOnly -and -not $Background) { Write-Host $_.ToString() }
            $_.ToString()
        } | Out-File -LiteralPath $log -Append
        $ErrorActionPreference = 'Stop'
        if ($LASTEXITCODE -ne 0) { throw "Could not build Gavnel. See $log" }
    } finally { Pop-Location }
    $built = Join-Path $projectRoot 'target\launcher\release\gavnel.exe'
    # Use .NET directly: Windows PowerShell launched from newer hosts can
    # inherit a module path where Get-FileHash cannot be loaded.
    $hashStream = [System.IO.File]::OpenRead($built)
    $sha256 = [System.Security.Cryptography.SHA256]::Create()
    try { $hash = [BitConverter]::ToString($sha256.ComputeHash($hashStream)).Replace('-', '') }
    finally { $sha256.Dispose(); $hashStream.Dispose() }
    $runDir = Join-Path $projectRoot "target\launcher\runs\$hash"
    $app = Join-Path $runDir 'Gavnel.exe'
    New-Item -ItemType Directory -Force $runDir | Out-Null
    if (-not (Test-Path -LiteralPath $app)) { Copy-Item -LiteralPath $built -Destination $app }
    # Publish only a complete versioned executable, including after BuildOnly.
    $ready = Join-Path $projectRoot 'target\launcher\ready.txt'
    [System.IO.File]::WriteAllText("$ready.tmp", $hash)
    Move-Item -LiteralPath "$ready.tmp" -Destination $ready -Force
    if ($BuildOnly) { exit 0 }
    if ($OnboardingPreview) {
        Start-Process -FilePath $app -ArgumentList '--onboarding-preview' -WorkingDirectory $projectRoot -WindowStyle Normal
        exit 0
    }
    $running = @(Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -like '*gavnel*' -and $_.Path -and $_.Path.StartsWith($projectRoot + '\', [StringComparison]::OrdinalIgnoreCase) })
    $older = @($running | Where-Object { $_.Path -ne $app })
    if ($older.Count -gt 0) {
        try { $exitEvent = [System.Threading.EventWaitHandle]::OpenExisting('Local\Gavnel.ExitForUpdate') }
        catch { throw 'An older Gavnel is still open. Choose Exit from its tray menu, then launch Gavnel again. Future versions will restart automatically.' }
        try { [void]$exitEvent.Set() } finally { $exitEvent.Dispose() }
        foreach ($process in $older) {
            if (-not $process.WaitForExit(60000)) { throw 'Gavnel is still finishing its recording. Please wait, then launch again.' }
        }
    }
    # Hide only tray startup. SW_HIDE can suppress the GUI's first ShowWindow.
    $start = @{ FilePath = $app; WorkingDirectory = $projectRoot; WindowStyle = 'Normal' }
    if ($Background) {
        $start.ArgumentList = '--background'
        $start.WindowStyle = 'Hidden'
    }
    $launched = Start-Process @start -PassThru
    "[$(Get-Date -Format o)] Started PID $($launched.Id): $app" | Add-Content -LiteralPath $log
    if ($launched.WaitForExit(3000)) {
        "[$(Get-Date -Format o)] Startup process exited with code $($launched.ExitCode)" | Add-Content -LiteralPath $log
        if ($launched.ExitCode -ne 0) { throw "Gavnel exited during startup (code $($launched.ExitCode)). See $log" }
    }
} catch {
    $_ | Out-String | Add-Content -LiteralPath $log
    if ($BuildOnly) { [Console]::Error.WriteLine($_.Exception.Message) }
    else { [void][System.Windows.Forms.MessageBox]::Show($_.Exception.Message, 'Gavnel launcher') }
    exit 1
} finally {
    if ($locked) { $lock.ReleaseMutex() }
    $lock.Dispose()
}



