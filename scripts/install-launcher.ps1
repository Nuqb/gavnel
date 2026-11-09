$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$shell = New-Object -ComObject WScript.Shell
$destinations = @(
    (Join-Path ([Environment]::GetFolderPath('Desktop')) 'Gavnel.lnk'),
    (Join-Path ([Environment]::GetFolderPath('Programs')) 'Gavnel.lnk')
)
foreach ($destination in $destinations) {
    $link = $shell.CreateShortcut($destination)
    $link.TargetPath = "$env:SystemRoot\System32\wscript.exe"
    $link.Arguments = '//B //Nologo "' + (Join-Path $PSScriptRoot 'launch.vbs') + '"'
    $link.WorkingDirectory = $projectRoot
    $link.IconLocation = Join-Path $projectRoot 'assets\gavnel.ico'
    $link.Save()
}
