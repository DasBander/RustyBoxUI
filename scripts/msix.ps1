param([Parameter(Mandatory)][string]$App, [Parameter(Mandatory)][string]$ConfigPath, [Parameter(Mandatory)][string]$Destination)
$ErrorActionPreference = 'Stop'
$Config = [IO.File]::ReadAllText($ConfigPath) | ConvertFrom-Json
if ($Config.identity -notmatch '^[A-Za-z0-9.-]{3,50}$' -or $Config.version -notmatch '^\d+\.\d+\.\d+\.\d+$') { throw 'Invalid MSIX identity or four-part version in rustybox.json' }
$kits = Join-Path ([Environment]::GetFolderPath('ProgramFilesX86')) 'Windows Kits\10\bin'
$makeappx = Get-ChildItem -LiteralPath $kits -Directory | Where-Object { $_.Name -match '^\d+\.\d+\.\d+\.\d+$' } | Sort-Object { [version]$_.Name } -Descending |
    ForEach-Object { Join-Path $_.FullName 'x64\makeappx.exe' } | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
if (-not $makeappx) { throw 'Install the Windows SDK to create an MSIX' }
$staging = New-Item -ItemType Directory -Path (Join-Path ([IO.Path]::GetTempPath()) ('rustybox-msix-' + [guid]::NewGuid().ToString('N')))
$buildId = [guid]::NewGuid().ToString('N')
$msixPath = Join-Path $Destination "$buildId.msix"
$zipPath = Join-Path $Destination "$buildId.zip"
try {
    Get-ChildItem -LiteralPath $App -Force | Copy-Item -Destination $staging.FullName -Recurse
    $art = New-Item -ItemType Directory -Path (Join-Path $staging.FullName 'AppIcons')
    & (Join-Path $PSScriptRoot 'cargo-rustybox.exe') msix-icons $ConfigPath $art.FullName
    if ($LASTEXITCODE) { throw 'Could not generate installer logos from the app icon' }
    $name=[Security.SecurityElement]::Escape([string]$Config.name)
    $publisher=[Security.SecurityElement]::Escape([string]$Config.publisher)
    $display=[Security.SecurityElement]::Escape([string]$Config.publisherDisplayName)
    $description = if ($Config.description) { [Security.SecurityElement]::Escape([string]$Config.description) } else { $name }
    $hosts = @(Get-ChildItem -LiteralPath $App -Filter '*.exe' -File)
    if ($hosts.Count -ne 1) { throw 'Expected a single executable in the portable application directory' }
    $appExe = [Security.SecurityElement]::Escape($hosts[0].Name)
    # The branding helper validated the extensions and schemes before writing the tiles.
    $extensions = @()
    if ($Config.fileTypes) {
        $types = ($Config.fileTypes | ForEach-Object { "<uap:FileType>$_</uap:FileType>" }) -join ''
        $extensions += '<uap:Extension Category="windows.fileTypeAssociation"><uap3:FileTypeAssociation Name="documents" Parameters="-- &quot;%1&quot;"><uap:SupportedFileTypes>' + $types + '</uap:SupportedFileTypes></uap3:FileTypeAssociation></uap:Extension>'
    }
    foreach ($scheme in $Config.protocols) {
        $extensions += '<uap3:Extension Category="windows.protocol"><uap3:Protocol Name="' + $scheme + '" Parameters="-- &quot;%1&quot;"/></uap3:Extension>'
    }
    $extensionXml = if ($extensions.Count) { '<Extensions>' + ($extensions -join '') + '</Extensions>' } else { '' }
    $manifest=@"
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10" xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10" xmlns:uap3="http://schemas.microsoft.com/appx/manifest/uap/windows10/3" xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities" IgnorableNamespaces="uap uap3 rescap">
  <Identity Name="$($Config.identity)" Publisher="$publisher" Version="$($Config.version)" ProcessorArchitecture="x64"/>
  <Properties><DisplayName>$name</DisplayName><PublisherDisplayName>$display</PublisherDisplayName><Logo>AppIcons\50.png</Logo></Properties>
  <Dependencies><TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.17763.0" MaxVersionTested="10.0.26100.0"/></Dependencies>
  <Resources><Resource Language="en-us"/></Resources>
  <Applications><Application Id="App" Executable="$appExe" EntryPoint="Windows.FullTrustApplication"><uap:VisualElements DisplayName="$name" Description="$description" BackgroundColor="transparent" Square150x150Logo="AppIcons\150.png" Square44x44Logo="AppIcons\44.png"/>$extensionXml</Application></Applications>
  <Capabilities><rescap:Capability Name="runFullTrust"/></Capabilities>
</Package>
"@
    [IO.File]::WriteAllText((Join-Path $staging.FullName 'AppxManifest.xml'),$manifest,[Text.UTF8Encoding]::new($false))
    $base=Join-Path $Destination ($Config.identity + '-' + $Config.version)
    & $makeappx pack /d $staging.FullName /p $msixPath /o | Out-Null
    if ($LASTEXITCODE) {throw 'MSIX packaging failed'}
    if ($env:RUSTYBOX_SIGN_THUMBPRINT) {
        $signtool=Join-Path (Split-Path -Parent $makeappx) 'signtool.exe'
        $signingArgs = @('sign', '/fd', 'SHA256', '/sha1', $env:RUSTYBOX_SIGN_THUMBPRINT)
        if ($env:RUSTYBOX_TIMESTAMP_URL) {
            $signingArgs += @('/tr', $env:RUSTYBOX_TIMESTAMP_URL, '/td', 'SHA256')
        }
        & $signtool @signingArgs $msixPath
        if ($LASTEXITCODE) {throw 'MSIX signing failed'}
    }
    # ZipFile normalizes old timestamps in vendored license files on Windows PowerShell.
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    [IO.Compression.ZipFile]::CreateFromDirectory($App, $zipPath, 'Optimal', $false)
    foreach ($pair in @(@($msixPath, "$base.msix"), @($zipPath, "$base-portable.zip"))) {
        if ([IO.File]::Exists($pair[1])) {
            [IO.File]::Replace($pair[0], $pair[1], [NullString]::Value)
        } else {
            [IO.File]::Move($pair[0], $pair[1])
        }
    }
    Write-Output "Created $base.msix and $base-portable.zip"
} finally {
    foreach ($temporaryFile in $msixPath, $zipPath) {
        if ([IO.File]::Exists($temporaryFile)) { [IO.File]::Delete($temporaryFile) }
    }
    $root=[IO.Path]::GetTempPath().TrimEnd('\')
    if ((Split-Path -Parent $staging.FullName) -eq $root -and $staging.Name -match '^rustybox-msix-[a-f0-9]{32}$') {Remove-Item -LiteralPath $staging.FullName -Recurse -Force}
}
