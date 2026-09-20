# SPDX-License-Identifier: GPL-3.0-or-later
# See LICENSE-EXCEPTION.md for the TFM2 linking exception and attribution terms.

param(
    [string]$SdkDir = $env:TFM2_MOD_SDK_STABLE,
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

# Public packaging stays locked until the versioned documentation is reconciled
# with the restored feature set. Foundation validation is intentionally separate.
& (Join-Path $PSScriptRoot "validate_repo.ps1") -ReleaseMetadata

if (-not $SkipBuild) {
    & (Join-Path $root "build_local.ps1") -SdkDir $SdkDir
    if ($LASTEXITCODE -ne 0) {
        throw "Release build failed with exit code $LASTEXITCODE."
    }
}

$buildInfoPath = Join-Path $root "tfm2_better_mod_menu.build.json"
if (-not (Test-Path -LiteralPath $buildInfoPath -PathType Leaf)) {
    throw "Build manifest is missing. Run build_local.ps1 before packaging."
}
$buildInfo = Get-Content -LiteralPath $buildInfoPath -Raw | ConvertFrom-Json
if ($buildInfo.source_dirty) {
    throw "Release packages must be built from a clean source tree."
}

$modInfo = Get-Content -LiteralPath (Join-Path $root "mod.mod_info") -Raw | ConvertFrom-Json
$buildRoot = Join-Path $root "builds"
$releaseRoot = Join-Path $buildRoot "tfm2-better-mod-menu-v$($modInfo.version)"
$runtimeRoot = Join-Path $releaseRoot $modInfo.mod_id
$archive = Join-Path $buildRoot "tfm2-better-mod-menu-v$($modInfo.version).zip"

if (Test-Path -LiteralPath $releaseRoot) {
    $resolvedReleaseRoot = (Resolve-Path -LiteralPath $releaseRoot).Path
    $expectedReleaseRoot = [System.IO.Path]::GetFullPath($releaseRoot)
    if (-not $resolvedReleaseRoot.Equals($expectedReleaseRoot, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to replace an unexpected release directory: $resolvedReleaseRoot"
    }
    Remove-Item -LiteralPath $resolvedReleaseRoot -Recurse -Force
}
if (Test-Path -LiteralPath $archive) {
    Remove-Item -LiteralPath $archive -Force
}

New-Item -ItemType Directory -Path $runtimeRoot -Force | Out-Null
& (Join-Path $PSScriptRoot "install_local.ps1") `
    -DestinationDir $runtimeRoot `
    -SkipBuild

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
Get-ChildItem -LiteralPath (Join-Path $root "assets\previews") -File |
    Copy-Item -Destination $docsPreviews

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
