param([Parameter(Mandatory)][string]$Sdk)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'showcases.ps1')
$runtime=New-Item -ItemType Directory -Path (Join-Path $Sdk 'runtime')
foreach($file in ($runtimeDlls + $rustHosts + @('rustybox-cpp-host.exe','LICENSE','licenses'))) {
    Copy-Item -LiteralPath (Join-Path $Sdk $file) -Destination $runtime.FullName -Recurse
}
$groups=[ordered]@{'ui'=@('launcher','cpp-host'); 'atlas-ui'=@('atlas'); 'starter-ui'=@('starter'); 'steam-ui'=@('steam')}
foreach($name in $showcaseApps){$groups["$name-ui"]=@($name)}
foreach($folder in $groups.Keys){
    $assets=Join-Path $Sdk $folder
    $hosts=@($groups[$folder] | ForEach-Object {Join-Path $runtime.FullName "rustybox-$_.exe"})
    $options = @()
    if ($folder -eq 'starter-ui') { $options = @('--manifest', (Join-Path $Sdk 'examples\starter\rustybox.json')) }
    & (Join-Path $Sdk 'cargo-rustybox.exe') seal @options $assets (Join-Path $runtime.FullName "$folder.rbpak") @hosts
    if($LASTEXITCODE){throw "Could not seal $folder"}
    foreach($credit in (Get-ChildItem -LiteralPath $assets -File -Recurse | Where-Object {$_.Name -match '(^CREDITS\.md$|[-_]OFL\.txt$|^LICENSE)'})){
        $relative=$credit.FullName.Substring($assets.Length+1)
        $dest=Join-Path $runtime.FullName "licenses\assets\$folder\$relative"
        New-Item -ItemType Directory -Path (Split-Path -Parent $dest) -Force | Out-Null
        Copy-Item -LiteralPath $credit.FullName -Destination $dest
    }
}
