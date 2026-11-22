param(
    [ValidateSet('debug', 'release')][string]$Configuration = 'release',
    [switch]$SkipCpp
)

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'showcases.ps1')
$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
if (Test-Path -LiteralPath $cargoBin) {
    $env:Path = "$cargoBin;$env:Path"
}

Push-Location $projectRoot
try {
    $cargoArgs = @('build', '--workspace', '--locked')
    if ($Configuration -eq 'release') {
        $cargoArgs += @('--release', '--config', (Join-Path $PSScriptRoot 'windows-release.toml'))
        $paths = & (Join-Path $PSScriptRoot 'cargo-paths.ps1') -ProjectRoot $projectRoot
        $cargoArgs += @('--config', $paths)
    }
    & cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) { throw 'Rust build failed' }

    $output = Join-Path $projectRoot "target\$Configuration"
    if ($Configuration -eq 'release') {
        # Cargo places the static-CRT build under the explicit MSVC target triple.
        $compiled = Join-Path $projectRoot 'target\x86_64-pc-windows-msvc\release'
        New-Item -ItemType Directory -Path $output -Force | Out-Null
        foreach ($file in ($runtimeDlls + $importLibraries + $rustHosts)) {
            Copy-Item -LiteralPath (Join-Path $compiled $file) -Destination $output -Force
        }
    }
    Copy-AppAssets $projectRoot $output
    if (-not $SkipCpp) {
        & (Join-Path $PSScriptRoot 'build-cpp.ps1') -Configuration $Configuration
    }
    Write-Output "Built $Configuration DLLs and hosts in $output"
} finally {
    Pop-Location
}
