param(
    [string]$Destination = (Join-Path $env:LOCALAPPDATA 'Programs\RustyBoxUI\0.7.0'),
    [switch]$CheckOnly,
    [string]$RuntimeInstaller
)

$ErrorActionPreference = 'Stop'
$source = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'showcases.ps1')
if (-not [Environment]::Is64BitOperatingSystem -or $env:PROCESSOR_ARCHITECTURE -ne 'AMD64') {
    throw 'Use 64-bit PowerShell on Windows x64.'
}
$assetFolders = @('ui', 'atlas-ui', 'starter-ui', 'steam-ui') + @($showcaseApps | ForEach-Object { "$_-ui" })
$sdkRoot = $source
$source = Join-Path $source "runtime"
$assetsToInstall = @($assetFolders | ForEach-Object { "$_.rbpak" })
$hostsToInstall = $rustHosts + 'rustybox-cpp-host.exe'
$requiredFiles = @('LICENSE', 'licenses\DEPENDENCIES.txt') + $runtimeDlls + $hostsToInstall
$requiredFiles += $assetsToInstall
foreach ($file in $requiredFiles) {
    if (-not (Test-Path -LiteralPath (Join-Path $source $file) -PathType Leaf)) {
        throw "Incomplete SDK bundle: $file"
    }
}
if (-not $CheckOnly) {
    $destination = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Destination).TrimEnd('\')
    $parent = Split-Path -Parent $destination
    if (-not $parent -or $destination.StartsWith($sdkRoot.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase) -or $destination -eq $sdkRoot) {
        throw 'Choose an installation directory outside the extracted SDK.'
    }
    if (Test-Path -LiteralPath $destination) {
        throw "Installation destination already exists: $destination"
    }
}

function Get-WebRuntime {
    $client = '{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
    foreach ($key in "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\$client", "HKCU:\Software\Microsoft\EdgeUpdate\Clients\$client") {
        $version = (Get-ItemProperty -LiteralPath $key -Name pv -ErrorAction SilentlyContinue).pv
        $parsed = $null
        if ([version]::TryParse([string]$version, [ref]$parsed) -and $parsed -gt [version]'0.0.0.0') {
            return $version
        }
    }
}

$version = Get-WebRuntime
if (-not $version) {
    if ($CheckOnly) {
        throw 'Microsoft WebView2 Evergreen Runtime is missing.'
    }
    $temporary = $null
    try {
        if (-not $RuntimeInstaller) {
            $temporary = Join-Path ([IO.Path]::GetTempPath()) ('rustybox-webview2-' + [guid]::NewGuid() + '.exe')
            Invoke-WebRequest -UseBasicParsing -Uri 'https://go.microsoft.com/fwlink/p/?LinkId=2124703' -OutFile $temporary
            $RuntimeInstaller = $temporary
        }
        $signature = Get-AuthenticodeSignature -LiteralPath $RuntimeInstaller
        if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch '(^|,\s*)O=Microsoft Corporation(,|$)') {
            throw 'WebView2 installer must have a valid Microsoft signature'
        }
        $installerProcess = Start-Process -FilePath $RuntimeInstaller -ArgumentList '/silent', '/install' -WindowStyle Hidden -PassThru -Wait
        $version = Get-WebRuntime
        if ($installerProcess.ExitCode -ne 0 -or !$version) {
            throw "WebView2 installation failed ($($installerProcess.ExitCode))"
        }
    } finally {
        if ($temporary -and (Test-Path -LiteralPath $temporary)) {
            Remove-Item -LiteralPath $temporary
        }
    }
}
Write-Output "WebView2 $version"
if ($CheckOnly) {
    return
}
New-Item -ItemType Directory -Force -Path $parent | Out-Null
$staging = Join-Path $parent ('rustybox-install-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $staging | Out-Null
try {
    $installedFiles = @('LICENSE', 'licenses') + $runtimeDlls + $hostsToInstall + $assetsToInstall
    $installedFiles += @(Get-ChildItem -LiteralPath $source -Directory | Where-Object {
        $_.Name -match '^(.+\.rbpak)\.chunks-[0-9a-f]{24}$' -and $assetsToInstall -contains $Matches[1]
    } | ForEach-Object Name)
    foreach ($file in $installedFiles) {
        Copy-Item -LiteralPath (Join-Path $source $file) -Destination $staging -Recurse
    }
    # Both paths are verified direct children of the same installation parent.
    if ((Split-Path -Parent ([IO.Path]::GetFullPath($staging))) -ne $parent -or (Split-Path -Parent $destination) -ne $parent) {
        throw 'Invalid staging path'
    }
    [IO.Directory]::Move($staging, $destination)
} finally {
    if ((Test-Path -LiteralPath $staging) -and (Split-Path -Parent ([IO.Path]::GetFullPath($staging))) -eq $parent -and (Split-Path -Leaf $staging) -match '^rustybox-install-[a-f0-9]{32}$') {
        Remove-Item -LiteralPath $staging -Recurse -Force
    }
}
Write-Output "Installed RustyBox applications to $destination"
