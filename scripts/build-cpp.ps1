param([ValidateSet('debug', 'release')][string]$Configuration = 'release')

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (-not (Test-Path -LiteralPath $vswhere)) {
    throw 'Visual Studio C++ Build Tools are required for the C++ host.'
}
$vsPath = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vsPath) { throw 'No MSVC x64 toolchain found.' }

Import-Module (Join-Path $vsPath 'Common7\Tools\Microsoft.VisualStudio.DevShell.dll')
Enter-VsDevShell -VsInstallPath $vsPath -SkipAutomaticLocation -DevCmdArguments '-arch=x64 -host_arch=x64' | Out-Null
$output = Join-Path $projectRoot "target\$Configuration"
New-Item -ItemType Directory -Force -Path $output | Out-Null
$compilerArgs = @(
    '/nologo', '/std:c++20', '/MT', '/EHsc', '/utf-8', '/W4', '/O2'
    "/I$projectRoot\include", "$projectRoot\examples\cpp-host\main.cpp"
    "/Fo$output\cpp-host.obj", "/Fe$output\rustybox-cpp-host.exe"
    '/link', 'user32.lib', 'windowsapp.lib'
)
& cl.exe @compilerArgs
if ($LASTEXITCODE -ne 0) { throw 'C++ host build failed' }

if ($Configuration -eq 'release') {
    . (Join-Path $PSScriptRoot 'showcases.ps1')
    foreach ($file in ($runtimeDlls + $rustHosts + 'rustybox-cpp-host.exe')) {
        $imports = & dumpbin.exe /nologo /dependents (Join-Path $output $file)
        if ($LASTEXITCODE -ne 0) { throw "Cannot inspect dependencies of $file" }
        $redist = $imports | Where-Object { $_ -match '^\s+(?:vcruntime|msvcp|msvcr\d|concrt)\S*\.dll\s*$' }
        if ($redist) { throw "$file requires an external Visual C++ runtime: $($redist -join ', ')" }
    }
    Write-Output "Verified static C runtime linkage for both DLLs and all $($rustHosts.Count + 1) hosts."
}
