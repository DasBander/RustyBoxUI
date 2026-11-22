$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent $PSScriptRoot
& (Join-Path $project 'build.ps1')
$release = New-Item -ItemType Directory -Path (Join-Path $project 'dist') -Force
$manifest = Join-Path $project 'rustybox.json'
& (Join-Path $PSScriptRoot 'msix.ps1') -App (Join-Path $project 'out') -ConfigPath $manifest -Destination $release.FullName
