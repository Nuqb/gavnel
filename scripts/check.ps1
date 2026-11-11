param([switch]$Quick)
$ErrorActionPreference = 'Stop'
Push-Location (Split-Path -Parent $PSScriptRoot)
try {
    python -S -m unittest discover -s scripts -p 'test_*.py'
    if ($LASTEXITCODE -ne 0) { throw 'Release and catalog tool tests failed.' }
    python scripts/check-locales.py
    if ($LASTEXITCODE -ne 0) { throw 'Localization checks failed.' }
    cargo fmt --check
    if ($LASTEXITCODE -ne 0) { throw 'Rust formatting checks failed.' }
    if (-not $Quick) {
        cargo test --locked
        if ($LASTEXITCODE -ne 0) { throw 'Rust tests failed.' }
        cargo clippy --locked --all-targets -- -D warnings
        if ($LASTEXITCODE -ne 0) { throw 'Rust lint checks failed.' }
    }
} finally { Pop-Location }
