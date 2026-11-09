$ErrorActionPreference='Stop'
$project=Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $project
try {
    & cargo run --offline -- --export-icons $PSScriptRoot
    if($LASTEXITCODE -ne 0){throw 'SVG icon export failed'}
} finally {Pop-Location}
Add-Type -AssemblyName System.Drawing
$icon=[System.Drawing.Icon]::new((Join-Path $PSScriptRoot 'gavnel.ico'),[System.Drawing.Size]::new(256,256))
try{
    $bitmap=$icon.ToBitmap()
    try{$bitmap.Save((Join-Path $PSScriptRoot 'gavnel-icon.png'),[System.Drawing.Imaging.ImageFormat]::Png)}finally{$bitmap.Dispose()}
}finally{$icon.Dispose()}
