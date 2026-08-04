# SPDX-License-Identifier: GPL-3.0-or-later
# See LICENSE-EXCEPTION.md for the TFM2 linking exception and attribution terms.

param(
    [string]$SdkDir = $env:TFM2_MOD_SDK,
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

& (Join-Path $PSScriptRoot "validate_repo.ps1")

if (-not $SkipBuild) {
    & (Join-Path $root "build_local.ps1") -SdkDir $SdkDir
    if ($LASTEXITCODE -ne 0) {
        throw "Release build failed with exit code $LASTEXITCODE."
    }
}

$dll = Join-Path $root "tfm2_better_mod_menu.dll"
if (-not (Test-Path -LiteralPath $dll -PathType Leaf)) {
    throw "tfm2_better_mod_menu.dll is missing."
}

$modInfo = Get-Content -LiteralPath (Join-Path $root "mod.mod_info") -Raw | ConvertFrom-Json
$buildRoot = Join-Path $root "builds"
$releaseRoot = Join-Path $buildRoot "tfm2-better-mod-menu-v$($modInfo.version)"
$runtimeRoot = Join-Path $releaseRoot $modInfo.mod_id
$archive = Join-Path $buildRoot "tfm2-better-mod-menu-v$($modInfo.version).zip"

if (Test-Path -LiteralPath $releaseRoot) {
    Remove-Item -LiteralPath $releaseRoot -Recurse -Force
}
if (Test-Path -LiteralPath $archive) {
    Remove-Item -LiteralPath $archive -Force
}

New-Item -ItemType Directory -Path $runtimeRoot -Force | Out-Null

foreach ($name in @(
    "tfm2_better_mod_menu.dll",
    "mod.mod_info",
    "mod.override_info",
    "better_mod_menu_profile.json",
    "profile_icon.png",
    "thumbnail.png",
    "LICENSE",
    "LICENSE-EXCEPTION.md",
    "NOTICE.md",
    "THIRD_PARTY_NOTICES.md"
)) {
    Copy-Item -LiteralPath (Join-Path $root $name) -Destination (Join-Path $runtimeRoot $name)
}

$docsRoot = Join-Path $releaseRoot "docs"
New-Item -ItemType Directory -Path $docsRoot -Force | Out-Null
foreach ($name in @(
    "better_mod_menu.schema.json",
    "better_mod_menu.json.example",
    "better_mod_menu_profile.json.example",
    "better_mod_menu_profile.schema.json",
    "README.md",
    "CHANGELOG.md",
    "MODDER_GUIDE.md"
)) {
    Copy-Item -LiteralPath (Join-Path $root $name) -Destination (Join-Path $docsRoot $name)
}
$docsPreviews = Join-Path $docsRoot "assets\previews"
New-Item -ItemType Directory -Path $docsPreviews -Force | Out-Null
Get-ChildItem -LiteralPath (Join-Path $root "assets/previews") -File |
    Copy-Item -Destination $docsPreviews

$runtimeUi = Join-Path $runtimeRoot "ui"
$runtimeLayouts = Join-Path $runtimeUi "layout"
$runtimeComponents = Join-Path $runtimeLayouts "mods_component"
New-Item -ItemType Directory -Path $runtimeComponents -Force | Out-Null
Copy-Item `
    -LiteralPath (Join-Path $root "ui/icons") `
    -Destination (Join-Path $runtimeUi "icons") `
    -Recurse
Copy-Item `
    -LiteralPath (Join-Path $root "ui/layout/better_mod_menu_runtime.ui") `
    -Destination (Join-Path $runtimeLayouts "better_mod_menu_runtime.ui")
foreach ($name in @(
    "bmm_mod_row_runtime.ui",
    "mod_file_cards_row_runtime.ui",
    "mod_setting_category_runtime.ui",
    "mod_setting_row_runtime.ui",
    "mod_slot_runtime.ui"
)) {
    Copy-Item `
        -LiteralPath (Join-Path $root "ui/layout/mods_component/$name") `
        -Destination (Join-Path $runtimeComponents $name)
}
New-Item -ItemType Directory -Path (Join-Path $runtimeRoot "third_party") -Force | Out-Null
Copy-Item `
    -LiteralPath (Join-Path $root "third_party/licenses") `
    -Destination (Join-Path $runtimeRoot "third_party/licenses") `
    -Recurse

$forbiddenReleaseFiles = @(
    Get-ChildItem -LiteralPath $releaseRoot -Recurse -File |
        Where-Object {
            $_.Name -like "*inspector*" -or
            $_.Name -like "better_mod_menu.dev*" -or
            $_.FullName -match '[\\/]ui[\\/]layout[\\/]dev[\\/]'
        }
)
if ($forbiddenReleaseFiles.Count -gt 0) {
    throw "Developer-only files entered the release payload: $($forbiddenReleaseFiles.FullName -join ', ')"
}

Compress-Archive `
    -Path (Join-Path $releaseRoot "*") `
    -DestinationPath $archive `
    -CompressionLevel Optimal
Write-Host "Release package created: $archive"
