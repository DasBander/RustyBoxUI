param([string]$Dotnet = 'dotnet', [string]$Runtime, [string]$Destination)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
if (!$Runtime) {
    $Runtime = if (Test-Path (Join-Path $root 'rustybox_ui.dll')) { $root } else { Join-Path $root 'target\release' }
}
if (!$Destination) { $Destination = Join-Path $root 'dist' }
$Runtime = (Resolve-Path -LiteralPath $Runtime).Path
foreach ($name in 'rustybox_ui.dll', 'rustybox_graphics.dll') {
    if (!(Test-Path -LiteralPath (Join-Path $Runtime $name) -PathType Leaf)) { throw "Missing $name in $Runtime" }
}
$notices = Join-Path $Runtime 'licenses'
if (!(Test-Path -LiteralPath $notices -PathType Container)) {
    & node (Join-Path $PSScriptRoot 'notices.mjs') $notices
    if ($LASTEXITCODE) { throw 'Could not collect licensing notices' }
}
& $Dotnet pack (Join-Path $root 'bindings\dotnet\RustyBoxUI\RustyBoxUI.csproj') -c Release -o $Destination "-p:NativeRuntimeDir=$Runtime" "-p:NoticesDir=$notices"
if ($LASTEXITCODE) { throw 'Could not pack the .NET binding' }
