param([Parameter(Mandatory)][string]$Destination)
$ErrorActionPreference = 'Stop'
$sdk = Split-Path -Parent $PSScriptRoot
$target = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Destination)
if ((Test-Path -LiteralPath $target -PathType Leaf) -or ((Test-Path -LiteralPath $target) -and (Get-ChildItem -LiteralPath $target -Force))) {
    throw "Choose an empty destination: $target"
}
# In a checkout the release build and the template live in different directories.
$runtimeSource = $sdk
$notices = Join-Path $sdk 'licenses'
$fromCheckout = Test-Path -LiteralPath (Join-Path $sdk 'crates\ui\Cargo.toml')
if ($fromCheckout) {
    $runtimeSource = Join-Path $sdk 'target\release'
}
$runtimeFiles = @('rustybox_ui.dll', 'rustybox_graphics.dll')
foreach ($file in $runtimeFiles) {
    if (-not (Test-Path -LiteralPath (Join-Path $runtimeSource $file) -PathType Leaf)) {
        throw "Missing $file. Run .\scripts\build.ps1 from the source checkout, or extract the full Windows SDK."
    }
}
if ($fromCheckout) {
    $notices = Join-Path $runtimeSource 'licenses'
    $env:Path = (Join-Path $env:USERPROFILE '.cargo\bin') + ';' + $env:Path
    & node (Join-Path $PSScriptRoot 'notices.mjs') $notices
    if ($LASTEXITCODE -ne 0) {
        throw 'Could not collect dependency notices'
    }
}
New-Item -ItemType Directory -Path $target -Force | Out-Null
foreach ($entry in '.gitignore', 'Cargo.toml', 'build.rs', 'commands.json', 'src', 'ui') {
    Copy-Item -LiteralPath (Join-Path $sdk "examples\starter\$entry") -Destination $target -Recurse
}
$cargoConfig = Join-Path $target '.cargo'
New-Item -ItemType Directory -Path $cargoConfig | Out-Null
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'windows-release.toml') -Destination (Join-Path $cargoConfig 'config.toml')
Copy-Item -LiteralPath (Join-Path $sdk 'Cargo.lock') -Destination $target
$vendor = Join-Path $target 'vendor'
$dlls = Join-Path $target 'runtime'
New-Item -ItemType Directory -Force -Path $vendor, $dlls | Out-Null
foreach ($crate in 'core', 'sdk', 'assets', 'build') {
    Copy-Item -LiteralPath (Join-Path $sdk "crates\$crate") -Destination $vendor -Recurse
}

# The generated project owns its workspace and carries the SDK as a path dependency.
$manifest = Join-Path $target 'Cargo.toml'
$contents = [IO.File]::ReadAllText($manifest).Replace('../../crates/sdk', 'vendor/sdk').Replace('../../crates/build', 'vendor/build')
[IO.File]::WriteAllText($manifest, $contents + [Environment]::NewLine + '[workspace]' + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
foreach ($file in $runtimeFiles) {
    Copy-Item -LiteralPath (Join-Path $runtimeSource $file) -Destination $dlls
}
Copy-Item -LiteralPath $notices -Destination $target -Recurse
Copy-Item -LiteralPath (Join-Path $sdk 'LICENSE') -Destination (Join-Path $target 'licenses\RustyBoxUI-LICENSE.txt')
Copy-Item -LiteralPath (Join-Path $sdk 'ui\sdk') -Destination (Join-Path $target 'ui') -Recurse -Force
Copy-Item -LiteralPath (Join-Path $sdk 'ui\theme.css') -Destination (Join-Path $target 'ui') -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'generate-commands.mjs') -Destination $target
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'cargo-paths.ps1') -Destination $target
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'build-starter.ps1') -Destination (Join-Path $target 'build.ps1')
$toolsDir = New-Item -ItemType Directory -Path (Join-Path $target 'tools')
foreach ($name in 'dev.mjs', 'package-app.ps1', 'msix.ps1', 'publish.ps1') {
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot $name) -Destination $toolsDir.FullName
}
Copy-Item -LiteralPath (Join-Path $runtimeSource 'cargo-rustybox.exe') -Destination $toolsDir.FullName
$branding = New-Item -ItemType Directory -Path (Join-Path $target 'branding')
Copy-Item -LiteralPath (Join-Path $vendor 'build\src\icon\rustybox.png') -Destination (Join-Path $branding.FullName 'app.png')
$config = [IO.File]::ReadAllText((Join-Path $sdk 'examples\starter\rustybox.json')) | ConvertFrom-Json
$config | Add-Member -NotePropertyName icon -NotePropertyValue 'branding/app.png'
[IO.File]::WriteAllText((Join-Path $target 'rustybox.json'), ($config | ConvertTo-Json) + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
Write-Output "Created $target. Run its build.ps1 to produce out\rustybox-starter.exe."
