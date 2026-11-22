param([Parameter(Mandatory)][string]$App, [Parameter(Mandatory)][string]$Destination, [switch]$GameCatalog)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
New-Item -ItemType Directory -Force -Path $Destination | Out-Null
Get-ChildItem -LiteralPath (Join-Path $App 'ui') -Force | Copy-Item -Destination $Destination -Recurse -Force
Copy-Item -LiteralPath (Join-Path $root 'ui\sdk') -Destination $Destination -Recurse -Force
Copy-Item -LiteralPath (Join-Path $root 'ui\theme.css') -Destination $Destination -Force

$appDirectory = Get-Item -LiteralPath $App
if ($appDirectory.Parent.Name -eq 'showcase') {
    Get-ChildItem -File -LiteralPath (Join-Path $appDirectory.Parent.FullName 'common') | Copy-Item -Destination $Destination -Force
    $typeDir = Join-Path $Destination 'fonts'
    New-Item -ItemType Directory -Path $typeDir -Force | Out-Null
    Get-ChildItem -File -LiteralPath (Join-Path $appDirectory.Parent.FullName 'common\fonts') | Copy-Item -Destination $typeDir -Force
    $displayType = switch ($appDirectory.Name) {
        'material' {
            'instrument-serif-italic.woff2'
            'Instrument-Serif-OFL.txt'
        }
        'patcher' {
            'barlow-condensed-800-normal.woff2'
            'Barlow-Condensed-OFL.txt'
        }
    }
    foreach ($file in $displayType) {
        Copy-Item -LiteralPath (Join-Path $root "ui\art\type\$file") -Destination $typeDir -Force
    }
    if ($appDirectory.Name -eq 'talus') {
        $type = New-Item -ItemType Directory -Force -Path (Join-Path $Destination 'art\type')
        foreach ($name in 'barlow-condensed-800-italic.woff2', 'Barlow-Condensed-OFL.txt') {
            Copy-Item -LiteralPath (Join-Path $root "ui\art\type\$name") -Destination $type.FullName
        }
    }
    if ($appDirectory.Name -in 'racing','material','patcher') {
        $native = New-Item -ItemType Directory -Force -Path (Join-Path $Destination '_native')
        if ($appDirectory.Name -eq 'patcher') {
            Copy-Item -LiteralPath (Join-Path $appDirectory.Parent.FullName 'audio\ui\breakwater.wav') -Destination (Join-Path $native.FullName 'transmission.wav') -Force
            Copy-Item -LiteralPath (Join-Path $App 'relay-stations.json') -Destination $native.FullName -Force
        } else {
            foreach ($shader in 'fullscreen.hlsl',($appDirectory.Name + '.hlsl')) {
                Copy-Item -LiteralPath (Join-Path $appDirectory.Parent.FullName "src\$shader") -Destination $native.FullName -Force
            }
        }
    }
    if ($appDirectory.Name -eq 'patcher') {
        $artDir = Join-Path $Destination 'art'
        New-Item -ItemType Directory -Path $artDir -Force | Out-Null
        Copy-Item -LiteralPath (Join-Path $root 'ui\art\signal.webp') -Destination $artDir -Force
    }
}

if ($GameCatalog) {
    Copy-Item -LiteralPath (Join-Path $root 'ui\catalog.js') -Destination $Destination -Force
    $art = Join-Path $Destination 'art'
    New-Item -ItemType Directory -Path $art -Force | Out-Null
    Get-ChildItem -LiteralPath (Join-Path $root 'ui\art') -File | Where-Object { $_.Extension -in '.webp', '.md' } | Copy-Item -Destination $art -Force
    Copy-Item -LiteralPath (Join-Path $root 'ui\art\type') -Destination $art -Recurse -Force
}
