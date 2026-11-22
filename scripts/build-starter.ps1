param([switch]$Run)
$ErrorActionPreference = 'Stop'
$env:Path = (Join-Path $env:USERPROFILE '.cargo\bin') + ';' + $env:Path
Push-Location $PSScriptRoot
try {
    $paths = & (Join-Path $PSScriptRoot 'cargo-paths.ps1') -ProjectRoot $PSScriptRoot
    & cargo build --release --target-dir target --config $paths
    if ($LASTEXITCODE -ne 0) {
        throw 'Starter build failed'
    }
    $out = Join-Path $PSScriptRoot 'out'
    New-Item -ItemType Directory -Force -Path $out | Out-Null
    Copy-Item -LiteralPath 'target\x86_64-pc-windows-msvc\release\rustybox-starter.exe' -Destination $out -Force
    Get-ChildItem -LiteralPath 'runtime' -Filter '*.dll' | Copy-Item -Destination $out -Force
    Copy-Item -LiteralPath 'licenses' -Destination $out -Recurse -Force
    $web = Join-Path $out 'starter-ui'
    New-Item -ItemType Directory -Force -Path $web | Out-Null
    Get-ChildItem -LiteralPath 'ui' | Copy-Item -Destination $web -Recurse -Force
    if ($Run) {
        & (Join-Path $out 'rustybox-starter.exe')
    }
    Write-Output "Built $out\rustybox-starter.exe"
} finally {
    Pop-Location
}
