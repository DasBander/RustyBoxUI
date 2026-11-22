$showcaseApps = @('racing', 'couch', 'material', 'patcher', 'audio')
$showcaseHosts = @($showcaseApps | ForEach-Object { "rustybox-$_.exe" })
$rustHosts = @(
    'rustybox-launcher.exe'
    'rustybox-atlas.exe'
    'rustybox-starter.exe'
    'rustybox-steam.exe'
) + $showcaseHosts
$runtimeDlls = @('rustybox_ui.dll', 'rustybox_graphics.dll')
$importLibraries = @($runtimeDlls | ForEach-Object { "$_.lib" })

function Copy-ShowcaseUi([string]$Root, [string]$Output) {
    foreach ($app in $showcaseApps) {
        $source = Join-Path $Root "examples\showcase\$app"
        $target = Join-Path $Output "$app-ui"
        & (Join-Path $Root 'scripts\stage-ui.ps1') -App $source -Destination $target -GameCatalog:($app -eq 'couch')
    }
}

function Copy-AppAssets([string]$Root, [string]$Output) {
    Copy-Item -LiteralPath (Join-Path $Root 'ui') -Destination $Output -Recurse -Force
    foreach ($app in 'atlas', 'starter', 'steam') {
        $source = Join-Path $Root "examples\$app"
        $target = Join-Path $Output "$app-ui"
        & (Join-Path $Root 'scripts\stage-ui.ps1') -App $source -Destination $target -GameCatalog:($app -eq 'steam')
    }
    Copy-ShowcaseUi $Root $Output
}
