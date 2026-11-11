param([Parameter(Mandatory = $true)][string]$Installer)
$ErrorActionPreference = 'Stop'
$appMutex = $null
if ([Threading.Mutex]::TryOpenExisting('Local\Gavnel.DesktopApp', [ref]$appMutex)) {
    $appMutex.Dispose()
    throw 'A recorder is running. Installer verification must run in a clean session; it will not stop Gavnel.'
}
$directory = Join-Path ([IO.Path]::GetTempPath()) ('gavnel-native-install-' + [guid]::NewGuid())
try {
    $arguments = @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/NOSHORTCUTS=1', '/NOLAUNCH=1', '/NOREGISTRY=1', ('/DIR="' + $directory + '"'))
    $setup = Start-Process -FilePath $Installer -ArgumentList $arguments -WindowStyle Hidden -Wait -PassThru
    if ($setup.ExitCode -ne 0) { throw "Test installation failed: $($setup.ExitCode)" }
    if (Test-Path -LiteralPath (Join-Path $directory 'media')) { throw 'Installer still bundles external editing tools.' }
    $libraries = @(Get-ChildItem -LiteralPath $directory -Recurse -File -Filter '*.dll')
    if ($libraries.Count) { throw 'Expected the native app without bundled codec DLLs.' }
    & (Join-Path $PSScriptRoot 'test-media-runtime.ps1') -App (Join-Path $directory 'Gavnel.exe')
} finally {
    $uninstaller = Join-Path $directory 'unins000.exe'
    if (Test-Path -LiteralPath $uninstaller) {
        $uninstall = Start-Process -FilePath $uninstaller -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART') -WindowStyle Hidden -Wait -PassThru
        if ($uninstall.ExitCode -ne 0) { throw "Test uninstall failed: $($uninstall.ExitCode)" }
    }
}
