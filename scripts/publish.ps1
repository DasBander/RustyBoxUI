param([string]$Rollback)
$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent $PSScriptRoot
$config = [IO.File]::ReadAllText((Join-Path $project 'rustybox.json')) | ConvertFrom-Json
$utf8 = [Text.UTF8Encoding]::new($false)

function Get-Version([string]$Text) {
    if ($Text -notmatch '^\d+\.\d+\.\d+\.\d+$') { throw "Expected a four-part version, got '$Text'" }
    foreach ($part in $Text.Split('.')) { if ([uint64]$part -gt 65535) {throw "Version component is too large: $Text"} }
    return [version]$Text
}
function Get-NextRevision([version]$Version) {
    [int[]]$parts = $Version.ToString().Split('.')
    for ($i = 3; $i -ge 0; $i--) {
        if ($parts[$i] -lt 65535) { $parts[$i]++; return ($parts -join '.') }
        $parts[$i] = 0
    }
    throw 'All four feed version components are exhausted'
}
function Read-PackageIdentity([string]$Path) {
    $archive = [IO.Compression.ZipFile]::OpenRead($Path)
    try {
        $entry = $archive.GetEntry('AppxManifest.xml')
        if (!$entry) { throw 'Missing AppxManifest.xml in the update package' }
        $stream = $entry.Open()
        try {
            $xml = [xml]::new()
            $xml.XmlResolver = $null
            $xml.Load($stream)
            return $xml.Package.Identity
        } finally { $stream.Dispose() }
    } finally { $archive.Dispose() }
}
function Get-DescriptorSignature([byte[]]$Content, $Certificate) {
    $data = [Security.Cryptography.Pkcs.ContentInfo]::new($Content)
    $cms = [Security.Cryptography.Pkcs.SignedCms]::new($data, $true)
    $signer = [Security.Cryptography.Pkcs.CmsSigner]::new($Certificate)
    $signer.DigestAlgorithm = [Security.Cryptography.Oid]::new('2.16.840.1.101.3.4.2.1')
    $cms.ComputeSignature($signer, $true)
    $cms.CheckSignature($true)
    return ,$cms.Encode()
}
function Get-ContentHash([byte[]]$Content) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try { return [BitConverter]::ToString($sha.ComputeHash($Content)).Replace('-', '').ToLowerInvariant() }
    finally { $sha.Dispose() }
}
function Write-Immutable([string]$Source, [string]$Destination) {
    if ([IO.File]::Exists($Destination)) {
        if ((Get-FileHash -LiteralPath $Source).Hash -ne (Get-FileHash -LiteralPath $Destination).Hash) {
            throw "A different package already uses this version. Increase version in rustybox.json."
        }
        return
    }
    [IO.File]::Move($Source, $Destination)
}

if ($config.identity -cnotmatch '^[A-Za-z0-9.-]{3,50}$') { throw 'Invalid package identity' }
if (!$config.updates.uri) { throw 'Set updates.uri in rustybox.json to the public HTTPS address ending in .appinstaller' }
$uri = [uri]$config.updates.uri
if (!$uri.IsAbsoluteUri -or ($uri.Scheme -ne 'https' -and !($uri.IsLoopback -and $uri.Scheme -eq 'http')) -or
    $uri.UserInfo -or $uri.Query -or $uri.Fragment -or $uri.Segments[-1] -notmatch '^[A-Za-z0-9][A-Za-z0-9._-]*\.appinstaller$') {
    throw 'Use an HTTPS .appinstaller URI with a filename. HTTP is supported on loopback for local testing.'
}
$hours = 12
if ($null -ne $config.updates.hoursBetweenUpdateChecks) {
    $hours = [uint32]::Parse([string]$config.updates.hoursBetweenUpdateChecks)
    if ($hours -gt 255) {throw 'hoursBetweenUpdateChecks must be between 0 and 255'}
}
$thumb = ($env:RUSTYBOX_SIGN_THUMBPRINT -replace '\s','')
if ($thumb -notmatch '^[A-Fa-f0-9]{40}$') { throw 'Set RUSTYBOX_SIGN_THUMBPRINT to the code-signing certificate in CurrentUser\My' }
$certificate = Get-Item -LiteralPath "Cert:\CurrentUser\My\$thumb"
if (!$certificate.HasPrivateKey) { throw 'The release signing certificate requires an accessible private key' }
$kits = Join-Path ([Environment]::GetFolderPath('ProgramFilesX86')) 'Windows Kits\10\bin'
$signtool = Get-ChildItem -LiteralPath $kits -Directory | Where-Object {$_.Name -match '^\d+\.\d+\.\d+\.\d+$'} |
    Sort-Object {[version]$_.Name} -Descending | ForEach-Object {Join-Path $_.FullName 'x64\signtool.exe'} | Where-Object {[IO.File]::Exists($_)} | Select-Object -First 1
if (!$signtool) { throw 'Install the Windows SDK signing tools' }

Add-Type -AssemblyName System.Security
Add-Type -AssemblyName System.IO.Compression.FileSystem
$dist = (New-Item -ItemType Directory -Force -Path (Join-Path $project 'dist')).FullName
$output = (New-Item -ItemType Directory -Force -Path (Join-Path $dist 'updates')).FullName
$packages = (New-Item -ItemType Directory -Force -Path (Join-Path $output 'packages')).FullName
$signatures = (New-Item -ItemType Directory -Force -Path (Join-Path $output 'signatures')).FullName
$feed = Join-Path $output $uri.Segments[-1]
$target = if ($Rollback) {Get-Version $Rollback} else {Get-Version $config.version}
$filename = "$($config.identity)-$target.msix"
$published = Join-Path $packages $filename
$packageUri = [uri]::new($uri, "packages/$filename")
# An exclusive release lock and a final atomic descriptor replacement keep concurrent publishes consistent.
$lock = [IO.File]::Open((Join-Path $dist '.publish.lock'), 'OpenOrCreate', 'ReadWrite', 'None')
$stage = $null
try {
    $stage = (New-Item -ItemType Directory -Path (Join-Path $dist ('.publish-' + [guid]::NewGuid().ToString('N')))).FullName
    $copy = $null
    $old = $null
    if ([IO.File]::Exists($feed)) {
        $oldBytes = [IO.File]::ReadAllBytes($feed)
        $oldCms = [Security.Cryptography.Pkcs.SignedCms]::new([Security.Cryptography.Pkcs.ContentInfo]::new($oldBytes), $true)
        $oldCms.Decode([IO.File]::ReadAllBytes((Join-Path $signatures ((Get-ContentHash $oldBytes) + '.p7s'))))
        $oldCms.CheckSignature($true)
        if ($oldCms.SignerInfos.Count -ne 1 -or !$oldCms.SignerInfos[0].Certificate -or
            $oldCms.SignerInfos[0].Certificate.Thumbprint -ne $thumb) {
            throw 'This feed was signed by a different certificate. Use an independently verified new feed for a key change.'
        }
        $old = ([xml]$utf8.GetString($oldBytes)).AppInstaller
        if ($old.Uri -cne $uri.AbsoluteUri -or $old.MainPackage.Name -cne $config.identity -or $old.MainPackage.Publisher -cne $config.publisher) {
            throw 'The existing feed has a different URI or package identity'
        }
    }
    if ($Rollback) {
        if (!$old -or ![IO.File]::Exists($published)) { throw 'Rollback requires a retained release from this feed' }
        if ($target -gt (Get-Version $old.MainPackage.Version)) { throw 'Use publish for a newer package' }
        $candidate = $published
    } else {
        if ($old -and $target -lt (Get-Version $old.MainPackage.Version)) { throw 'Use rollback to select an older release' }
        $candidate = Join-Path $dist $filename
        if (![IO.File]::Exists($candidate)) { & (Join-Path $PSScriptRoot 'package-app.ps1') }
    }
    $identity = Read-PackageIdentity $candidate
    if ($identity.Name -cne $config.identity -or $identity.Publisher -cne $config.publisher -or
        (Get-Version $identity.Version) -ne $target -or $identity.ProcessorArchitecture -ne 'x64') {
        throw 'The MSIX identity, publisher, version or architecture does not match the feed'
    }
    $signed = Join-Path $stage $filename
    [IO.File]::Copy($candidate, $signed)
    $signature = Get-AuthenticodeSignature -LiteralPath $signed
    if (!$signature.SignerCertificate -and !$Rollback) {
        $argsSign = @('sign','/fd','SHA256','/sha1',$thumb)
        if ($env:RUSTYBOX_TIMESTAMP_URL) { $argsSign += @('/tr',$env:RUSTYBOX_TIMESTAMP_URL,'/td','SHA256') }
        & $signtool @argsSign $signed | Out-Null
        if ($LASTEXITCODE) {throw 'MSIX signing failed'}
        $signature = Get-AuthenticodeSignature -LiteralPath $signed
        # Reuse those exact signed bytes when publishing this release again.
        $copy = Join-Path $stage 'signed-copy.msix'
        [IO.File]::Copy($signed, $copy)
    }
    if (!$signature.SignerCertificate -or $signature.SignerCertificate.Thumbprint -ne $thumb) {throw 'Unexpected MSIX signer'}
    & $signtool verify /pa $signed | Out-Null
    if ($LASTEXITCODE) {throw 'Windows could not verify the MSIX publisher or package integrity'}
    if (!$Rollback) {
        Write-Immutable $signed $published
        if ($copy) { [IO.File]::Replace($copy,$candidate,[NullString]::Value) }
    }
    $recover = $Rollback -and $old -and $target -lt (Get-Version $old.MainPackage.Version)
    if ($old -and (Get-Version $old.MainPackage.Version) -eq $target -and
        $old.UpdateSettings.OnLaunch.HoursBetweenUpdateChecks -eq [string]$hours -and
        ($Rollback -or $old.UpdateSettings.ForceUpdateFromAnyVersion -ne 'true')) {
        Write-Output "Already published $feed"; return
    }
    $revision = $target.ToString()
    if ($old -and $target -le (Get-Version $old.Version)) { $revision = Get-NextRevision (Get-Version $old.Version) }
    $xmlns = 'http://schemas.microsoft.com/appx/appinstaller/2018'
    $doc = [xml]::new()
    $root = $doc.CreateElement('AppInstaller',$xmlns)
    [void]$doc.AppendChild($root)
    $root.SetAttribute('Uri',$uri.AbsoluteUri); $root.SetAttribute('Version',$revision)
    $main = $doc.CreateElement('MainPackage',$xmlns)
    foreach ($key in 'Name','Publisher','Version','ProcessorArchitecture') { $main.SetAttribute($key,$identity.GetAttribute($key)) }
    $main.SetAttribute('Uri',$packageUri.AbsoluteUri); [void]$root.AppendChild($main)
    $settings = $doc.CreateElement('UpdateSettings',$xmlns); [void]$root.AppendChild($settings)
    $launch = $doc.CreateElement('OnLaunch',$xmlns); $launch.SetAttribute('HoursBetweenUpdateChecks',[string]$hours); [void]$settings.AppendChild($launch)
    [void]$settings.AppendChild($doc.CreateElement('AutomaticBackgroundTask',$xmlns))
    if ($recover) { $force=$doc.CreateElement('ForceUpdateFromAnyVersion',$xmlns); $force.InnerText='true'; [void]$settings.AppendChild($force) }
    $bytes = $utf8.GetBytes($doc.OuterXml)
    $receipt = Join-Path $signatures ((Get-ContentHash $bytes) + '.p7s')
    $temporary = Join-Path $stage 'descriptor.tmp'
    [IO.File]::WriteAllBytes($temporary,$bytes)
    if (![IO.File]::Exists($receipt)) {
        $sigFile = Join-Path $stage 'signature.tmp'
        [IO.File]::WriteAllBytes($sigFile,(Get-DescriptorSignature $bytes $certificate))
        [IO.File]::Move($sigFile,$receipt)
    }
    if ([IO.File]::Exists($feed)) { [IO.File]::Replace($temporary,$feed,[NullString]::Value) }
    else { [IO.File]::Move($temporary,$feed) }
    Write-Output "Prepared $feed for $($uri.AbsoluteUri) with package $target, feed revision $revision"
} finally {
    $lock.Dispose()
    if ($stage -and [IO.Path]::GetDirectoryName($stage) -eq $dist -and [IO.Path]::GetFileName($stage) -match '^\.publish-[a-f0-9]{32}$') {
        Remove-Item -LiteralPath $stage -Recurse -Force
    }
}
