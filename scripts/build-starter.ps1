param([switch]$Run, [switch]$Development)
$ErrorActionPreference = 'Stop'
$env:Path = (Join-Path $env:USERPROFILE '.cargo\bin') + ';' + $env:Path
Push-Location $PSScriptRoot
try {
    # Validate the Windows filename before using it as an output path.
    $encoding = [Console]::OutputEncoding
    try {
        [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
        $exeName = & .\tools\cargo-rustybox.exe exe-name rustybox.json
        if ($LASTEXITCODE) { throw 'Invalid Windows application metadata' }
    } finally {
        [Console]::OutputEncoding = $encoding
    }
    $paths = & (Join-Path $PSScriptRoot 'cargo-paths.ps1') -ProjectRoot $PSScriptRoot
    & cargo build --release --target-dir target --config $paths
    if ($LASTEXITCODE) {throw 'Starter build failed'}
    $targetRoot = (Get-Item -LiteralPath target).FullName
    $stage = New-Item -ItemType Directory -Path (Join-Path $targetRoot ('stage-' + [guid]::NewGuid().ToString('N')))
    $exe = Join-Path $stage.FullName $exeName
    Copy-Item -LiteralPath 'target\x86_64-pc-windows-msvc\release\rustybox-starter.exe' -Destination $exe
    Get-ChildItem -LiteralPath runtime -Filter '*.dll' | Copy-Item -Destination $stage.FullName
    Copy-Item -LiteralPath licenses -Destination $stage.FullName -Recurse
    if ($Development) {
        Copy-Item -LiteralPath ui -Destination (Join-Path $stage.FullName 'starter-ui') -Recurse
        $out = Join-Path $targetRoot 'live'
    } else {
        & .\tools\cargo-rustybox.exe seal ui (Join-Path $stage.FullName 'starter-ui.rbpak') $exe
        if ($LASTEXITCODE) {throw 'Asset packaging failed'}
        # Font licenses stay readable in redistributable apps.
        $assetNotices=New-Item -ItemType Directory -Path (Join-Path $stage.FullName 'licenses\assets') -Force
        $uiRoot=(Get-Item -LiteralPath ui).FullName
        Get-ChildItem -LiteralPath $uiRoot -Recurse -File | Where-Object { $_.Name -match '(^CREDITS\.md$|[-_]OFL\.txt$|^LICENSE)' } | ForEach-Object {
            $notice=Join-Path $assetNotices.FullName ($_.FullName.Substring($uiRoot.Length+1))
            New-Item -ItemType Directory -Path (Split-Path -Parent $notice) -Force | Out-Null
            Copy-Item -LiteralPath $_.FullName -Destination $notice
        }
        $out = Join-Path $PSScriptRoot 'out'
    }
    $old=$null
    if (Test-Path -LiteralPath $out) {
        $previous = Get-Item -LiteralPath $out
        $assets = if ($Development) { 'starter-ui\index.html' } else { 'starter-ui.rbpak' }
        $hasRuntime = Test-Path -LiteralPath (Join-Path $out 'rustybox_ui.dll') -PathType Leaf
        $hasHost = @(Get-ChildItem -LiteralPath $out -File -Filter '*.exe').Count -eq 1
        $hasPayload = Test-Path -LiteralPath (Join-Path $out $assets) -PathType Leaf
        if (($previous.Attributes -band [IO.FileAttributes]::ReparsePoint) -or !$hasHost -or !$hasRuntime -or !$hasPayload) {
            throw "Choose an empty directory or a previous RustyBox build: $out"
        }
        $old=Join-Path $targetRoot ('prior-' + [guid]::NewGuid().ToString('N'))
        [IO.Directory]::Move($out,$old)
    }
    try { [IO.Directory]::Move($stage.FullName,$out) }
    catch { if($old){[IO.Directory]::Move($old,$out)}; throw }
    if ($old -and (Split-Path -Parent $old) -eq $targetRoot -and (Split-Path -Leaf $old) -match '^prior-[a-f0-9]{32}$') {
        Remove-Item -LiteralPath $old -Recurse -Force
    }
    $result = Join-Path $out $exeName
    Write-Output "Built $result"
    if ($Run) { & $result }
} finally {
    Pop-Location
}
