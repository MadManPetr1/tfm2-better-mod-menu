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

$dll = Join-Path $root "mod_menu.dll"
if (-not (Test-Path -LiteralPath $dll -PathType Leaf)) {
    throw "mod_menu.dll is missing."
}

$modInfo = Get-Content -LiteralPath (Join-Path $root "mod.mod_info") -Raw | ConvertFrom-Json
$buildRoot = Join-Path $root "builds"
$releaseRoot = Join-Path $buildRoot "better-mod-menu-v$($modInfo.version)"
$runtimeRoot = Join-Path $releaseRoot "mod_menu"
$archive = Join-Path $buildRoot "better-mod-menu-v$($modInfo.version).zip"

if (Test-Path -LiteralPath $releaseRoot) {
    Remove-Item -LiteralPath $releaseRoot -Recurse -Force
}
if (Test-Path -LiteralPath $archive) {
    Remove-Item -LiteralPath $archive -Force
}

New-Item -ItemType Directory -Path $runtimeRoot -Force | Out-Null

foreach ($name in @(
    "mod_menu.dll",
    "mod.mod_info",
    "mod.override_info",
    "better_mod_menu.schema.json",
    "README.md",
    "CHANGELOG.md",
    "LICENSE",
    "LICENSE-EXCEPTION.md",
    "NOTICE.md",
    "MANIFEST.md",
    "MODDER_GUIDE.md",
    "THIRD_PARTY_NOTICES.md"
)) {
    Copy-Item -LiteralPath (Join-Path $root $name) -Destination (Join-Path $runtimeRoot $name)
}

Copy-Item -LiteralPath (Join-Path $root "ui") -Destination (Join-Path $runtimeRoot "ui") -Recurse
New-Item -ItemType Directory -Path (Join-Path $runtimeRoot "third_party") -Force | Out-Null
Copy-Item `
    -LiteralPath (Join-Path $root "third_party/licenses") `
    -Destination (Join-Path $runtimeRoot "third_party/licenses") `
    -Recurse

Compress-Archive -LiteralPath $runtimeRoot -DestinationPath $archive -CompressionLevel Optimal
Write-Host "Release package created: $archive"
