param([string]$ToolsDirectory = (Join-Path $PSScriptRoot '../work/tools/inno'))
$ErrorActionPreference = 'Stop'
$compiler = Join-Path $ToolsDirectory 'ISCC.exe'
if (Test-Path -LiteralPath $compiler) { Write-Output ([IO.Path]::GetFullPath($compiler)); exit 0 }
$download = Join-Path ([IO.Path]::GetFullPath((Join-Path $ToolsDirectory '..'))) 'innosetup-7.1.1.exe'
New-Item -ItemType Directory -Force (Split-Path $download) | Out-Null
Invoke-WebRequest -Uri 'https://github.com/jrsoftware/issrc/releases/download/is-7_1_1/innosetup-7.1.1-x64.exe' -OutFile $download
$expected = '8a5a6711c3766a6e5fb74f280ba0e3551e719ce94538d901fdbd1f8f9920b3f0'
if ((Get-FileHash -LiteralPath $download -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expected) { throw 'Installer compiler checksum verification failed.' }
$setup = Start-Process -FilePath $download -ArgumentList @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART','/CURRENTUSER',('/DIR="' + [IO.Path]::GetFullPath($ToolsDirectory) + '"'),'/TASKS=') -WindowStyle Hidden -Wait -PassThru
if ($setup.ExitCode -ne 0 -or -not (Test-Path -LiteralPath $compiler)) { throw 'Could not install the release compiler.' }
Write-Output ([IO.Path]::GetFullPath($compiler))
