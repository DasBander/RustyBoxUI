param([switch]$SkipBuild, [string]$Destination)

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'showcases.ps1')
if (-not $SkipBuild) {
    & (Join-Path $PSScriptRoot 'build.ps1') -Configuration release
}
if (-not $Destination) {
    $Destination = Join-Path $projectRoot 'dist\RustyBoxUI-0.7.0'
}
$destination = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Destination).TrimEnd('\')
$parent = Split-Path -Parent $destination
$name = Split-Path -Leaf $destination
if (-not $parent -or -not $name -or (Test-Path -LiteralPath $destination -PathType Leaf)) {
    throw "Choose an SDK output directory: $destination"
}
if (Test-Path -LiteralPath $destination) {
    $existing = Get-Item -LiteralPath $destination
    $sdkDll = Join-Path $destination 'rustybox_ui.dll'
    $sdkManifest = Join-Path $destination 'Cargo.toml'
    if (($existing.Attributes -band [IO.FileAttributes]::ReparsePoint) -or
        ((Get-ChildItem -LiteralPath $destination -Force) -and
         (-not (Test-Path -LiteralPath $sdkDll -PathType Leaf) -or
          -not (Test-Path -LiteralPath $sdkManifest -PathType Leaf)))) {
        throw "Choose an empty directory or a previously generated SDK: $destination"
    }
}

# Assemble a fresh tree so files left in an older output can never enter the ZIP.
$stageRoot = Join-Path $parent ('.rustybox-package-' + [guid]::NewGuid().ToString('N'))
$bundle = Join-Path $stageRoot $name
New-Item -ItemType Directory -Path $bundle -Force | Out-Null
try {
    foreach ($file in ($runtimeDlls + $importLibraries + $rustHosts + @('rustybox-cpp-host.exe','cargo-rustybox.exe'))) {
        Copy-Item -LiteralPath (Join-Path $projectRoot "target\release\$file") -Destination $bundle
    }
    Copy-AppAssets $projectRoot $bundle
    foreach ($folder in 'crates', 'examples', 'scripts', '.cargo', 'screenshots') {
        New-Item -ItemType Directory -Path (Join-Path $bundle $folder) | Out-Null
    }
    foreach ($crate in 'core', 'sdk', 'assets', 'build') {
        $crateOut = New-Item -ItemType Directory -Path (Join-Path $bundle "crates\$crate")
        foreach ($item in 'src', 'Cargo.toml') {
            Copy-Item -LiteralPath (Join-Path $projectRoot "crates\$crate\$item") -Destination $crateOut.FullName -Recurse
        }
    }
    $examples = [ordered]@{
        starter = @('rustybox.json', 'build.rs', '.gitignore', 'Cargo.toml', 'src', 'ui', 'commands.json')
        atlas = @('Cargo.toml', 'src', 'ui', 'commands.json')
        steam = @('Cargo.toml', 'src', 'ui', 'commands.json')
        showcase = @('Cargo.toml', 'src', 'common') + $showcaseApps
        'cpp-host' = @('main.cpp')
    }
    foreach ($app in $examples.Keys) {
        $appOut = New-Item -ItemType Directory -Path (Join-Path $bundle "examples\$app")
        foreach ($item in $examples[$app]) {
            Copy-Item -LiteralPath (Join-Path $projectRoot "examples\$app\$item") -Destination $appOut.FullName -Recurse
        }
    }
    $sdkScripts = @(
        'stage-ui.ps1', 'showcases.ps1', 'new-app.ps1', 'build-starter.ps1'
        'dev.mjs', 'preview.mjs', 'package-app.ps1', 'msix.ps1', 'publish.ps1', 'generate-commands.mjs', 'install.ps1', 'windows-release.toml', 'cargo-paths.ps1'
    )
    foreach ($file in $sdkScripts) {
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot $file) -Destination (Join-Path $bundle 'scripts')
    }
    foreach ($item in 'include', 'README.md', 'LICENSE', 'docs', 'Cargo.lock', 'package.json') {
        Copy-Item -LiteralPath (Join-Path $projectRoot $item) -Destination $bundle -Recurse
    }
    Get-ChildItem -LiteralPath (Join-Path $bundle 'docs\images') -Filter '*.jpg' |
        Copy-Item -Destination (Join-Path $bundle 'screenshots')

    $env:Path = (Join-Path $env:USERPROFILE '.cargo\bin') + ';' + $env:Path
    & node (Join-Path $PSScriptRoot 'notices.mjs') (Join-Path $bundle 'licenses')
    if ($LASTEXITCODE -ne 0) { throw 'Could not collect dependency notices' }
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'windows-release.toml') -Destination (Join-Path $bundle '.cargo\config.toml')
    $manifest = @'
[workspace]
members = ["crates/assets", "crates/build", "crates/core", "crates/sdk", "examples/starter", "examples/atlas", "examples/steam", "examples/showcase"]
resolver = "2"
'@
    $manifestPath = Join-Path $bundle 'Cargo.toml'
    [IO.File]::WriteAllText($manifestPath, $manifest + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
    & cargo metadata --offline --manifest-path $manifestPath --format-version 1 | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Cannot resolve the standalone SDK workspace' }

    & (Join-Path $PSScriptRoot 'runtime.ps1') -Sdk $bundle
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archive = Join-Path $parent 'RustyBoxUI-0.7.0-windows-x64.zip'
    $stagedArchive = Join-Path $stageRoot 'release.zip'
    [IO.Compression.ZipFile]::CreateFromDirectory($bundle, $stagedArchive, 'Optimal', $true)

    $runtimeArchive = Join-Path $parent 'RustyBoxUI-0.7.0-apps-windows-x64.zip'
    $stagedRuntime = Join-Path $stageRoot 'runtime.zip'
    [IO.Compression.ZipFile]::CreateFromDirectory((Join-Path $bundle 'runtime'), $stagedRuntime, 'Optimal', $false)
    if ((Split-Path -Parent $bundle) -ne $stageRoot -or (Split-Path -Parent $destination) -ne $parent) {
        throw 'Invalid SDK staging paths'
    }
    if (Test-Path -LiteralPath $destination) {
        # Preserve the previous loose output, including any files the developer added to it.
        $previous = Join-Path $parent ($name + '.previous-' + [guid]::NewGuid().ToString('N'))
        [IO.Directory]::Move($destination, $previous)
        try {
            [IO.Directory]::Move($bundle, $destination)
        } catch {
            [IO.Directory]::Move($previous, $destination)
            throw
        }
        Write-Output "Previous SDK retained at $previous"
    } else {
        [IO.Directory]::Move($bundle, $destination)
    }
    if ([IO.File]::Exists($archive)) {
        [IO.File]::Replace($stagedArchive, $archive, [NullString]::Value)
    } else {
        [IO.File]::Move($stagedArchive, $archive)
    }
    if ([IO.File]::Exists($runtimeArchive)) {
        [IO.File]::Replace($stagedRuntime,$runtimeArchive,[NullString]::Value)
    } else { [IO.File]::Move($stagedRuntime,$runtimeArchive) }
    Write-Output "Packaged applications: $runtimeArchive"
    Write-Output "Packaged SDK: $archive"
} finally {
    if ((Split-Path -Parent ([IO.Path]::GetFullPath($stageRoot))) -eq $parent -and
        (Split-Path -Leaf $stageRoot) -match '^\.rustybox-package-[a-f0-9]{32}$' -and
        (Test-Path -LiteralPath $stageRoot)) {
        Remove-Item -LiteralPath $stageRoot -Recurse -Force
    }
}
