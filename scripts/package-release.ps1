param([string]$ExpectedVersion, [switch]$SkipBuild, [switch]$Resume, [string]$Compiler)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
$previousFlags = $env:RUSTFLAGS
try {
    $manifest = Get-Content -LiteralPath 'Cargo.toml' -Raw
    $version = [regex]::Match($manifest, '(?m)^version = "([0-9]+\.[0-9]+\.[0-9]+)"').Groups[1].Value
    if (-not $version -or ($ExpectedVersion -and $version -ne $ExpectedVersion)) { throw 'The release tag and Cargo version must match.' }
    $releaseRoot = Join-Path $projectRoot "work/releases/v$version"
    $payload = Join-Path $releaseRoot 'payload'
    if ((Test-Path -LiteralPath $payload) -and -not $Resume) { throw 'A release payload already exists. Use a clean checkout or a new version; pass -Resume only for a failed unpublished build.' }
    if (-not $SkipBuild) {
        $env:RUSTFLAGS = (($previousFlags + ' -C target-feature=+crt-static').Trim())
        cargo build --release --locked --target x86_64-pc-windows-msvc --target-dir target/distribution
        if ($LASTEXITCODE -ne 0) { throw 'Release build failed.' }
    }
    $binary = Join-Path $projectRoot 'target/distribution/x86_64-pc-windows-msvc/release/gavnel.exe'
    if (-not (Test-Path -LiteralPath $binary)) { throw 'The release executable is missing.' }
    # GUI executables need explicit pipes when launched from Windows PowerShell.
    $start = New-Object Diagnostics.ProcessStartInfo
    $start.FileName = $binary
    $start.Arguments = '--version'
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $process = [Diagnostics.Process]::Start($start)
    $reported = $process.StandardOutput.ReadToEnd().Trim()
    $process.WaitForExit()
    if ($process.ExitCode -ne 0 -or $reported -ne "Gavnel $version") { throw 'The built executable has the wrong version.' }
    New-Item -ItemType Directory -Force $payload | Out-Null
    Copy-Item -LiteralPath $binary -Destination (Join-Path $payload 'Gavnel.exe')
    foreach ($name in @('LICENSE','HISTORICAL-MIT.txt','THIRD_PARTY_NOTICES.md','README.md','CHANGELOG.md')) { Copy-Item -LiteralPath (Join-Path $projectRoot $name) -Destination $payload }
    Copy-Item -LiteralPath 'packaging/gavnel-install.txt' -Destination $payload
    python scripts/get-media.py --copy-to (Join-Path $payload 'media')
    if ($LASTEXITCODE -ne 0) { throw 'Editing runtime verification failed.' }
    python scripts/package-licenses.py $payload
    if ($LASTEXITCODE -ne 0) { throw 'License inventory generation failed.' }
    if (-not $Compiler) { $Compiler = & (Join-Path $PSScriptRoot 'get-inno.ps1') }
    & $Compiler /Q "/DAppVersion=$version" "/DPayloadDir=$payload" "/DOutputDir=$releaseRoot" packaging/gavnel.iss
    if ($LASTEXITCODE -ne 0) { throw 'Installer compilation failed.' }
    $installer = Join-Path $releaseRoot "Gavnel-Setup-$version-x64.exe"
    $checksum = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant()
    "$checksum  $([IO.Path]::GetFileName($installer))" | Set-Content -LiteralPath (Join-Path $releaseRoot 'SHA256SUMS.txt') -Encoding ascii
    Write-Output "Release ready: $installer"
} finally { $env:RUSTFLAGS = $previousFlags; Pop-Location }
