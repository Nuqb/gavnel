$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$shell = New-Object -ComObject WScript.Shell
$destination = Join-Path ([Environment]::GetFolderPath('Desktop')) 'Gavnel - Quick Launch.lnk'
$link = $shell.CreateShortcut($destination)
$link.TargetPath = "$env:SystemRoot\System32\wscript.exe"
$link.Arguments = '//B //Nologo "' + (Join-Path $PSScriptRoot 'launch.vbs') + '" --quick'
$link.WorkingDirectory = $projectRoot
$link.IconLocation = Join-Path $projectRoot 'assets\gavnel.ico'
$link.Description = 'Open the latest completed Gavnel build immediately. Unbuilt source changes are not included.'
$link.Save()
Write-Host "Created $destination"
