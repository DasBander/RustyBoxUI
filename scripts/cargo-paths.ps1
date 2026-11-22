param([Parameter(Mandatory)][string]$ProjectRoot)

# Rust records source locations for panic messages. Use stable paths in redistributable binaries.
$ErrorActionPreference = 'Stop'
$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
$flags = @(
    "--remap-path-prefix=$([IO.Path]::GetFullPath($ProjectRoot))=workspace"
    "--remap-path-prefix=$([IO.Path]::GetFullPath($cargoHome))=cargo"
)
$target = New-Item -ItemType Directory -Path (Join-Path $ProjectRoot 'target') -Force
$config = Join-Path $target.FullName 'release-paths.toml'
$encodedFlags = ConvertTo-Json -InputObject $flags -Compress
$content = "[target.x86_64-pc-windows-msvc]`nrustflags = $encodedFlags`n"
[IO.File]::WriteAllText($config, $content, [Text.UTF8Encoding]::new($false))
Write-Output $config
