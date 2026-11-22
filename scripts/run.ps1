param(
    [ValidateSet('debug', 'release')][string]$Configuration = 'release',
    [switch]$Cpp,
    [switch]$Atlas,
    [switch]$Starter,
    [switch]$Steam,
    [ValidateSet('racing', 'couch', 'material', 'patcher', 'audio')][string]$Example
)

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
if (@($Cpp, $Atlas, $Starter, $Steam, $Example).Where({ $_ }).Count -gt 1) {
    throw 'Choose one application'
}
$name = if ($Example) { $Example }
        elseif ($Cpp) { 'cpp-host' }
        elseif ($Atlas) { 'atlas' }
        elseif ($Starter) { 'starter' }
        elseif ($Steam) { 'steam' }
        else { 'launcher' }

$exe = Join-Path $projectRoot "target\$Configuration\rustybox-$name.exe"
if (-not (Test-Path -LiteralPath $exe)) {
    & (Join-Path $PSScriptRoot 'build.ps1') -Configuration $Configuration
}
# In a checkout the two storefront hosts can reload edits directly from ui/.
if ($name -in 'cpp-host', 'launcher') {
    $assets = Join-Path $projectRoot 'ui'
    Start-Process -FilePath $exe -ArgumentList @('--assets', ('"' + $assets + '"')) -WorkingDirectory $projectRoot
} else {
    Start-Process -FilePath $exe -WorkingDirectory $projectRoot
}
