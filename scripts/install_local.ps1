# SPDX-License-Identifier: GPL-3.0-or-later
# See LICENSE-EXCEPTION.md for the TFM2 linking exception and attribution terms.

param(
    [string]$SdkDir = $env:TFM2_MOD_SDK_STABLE,
    [string]$DestinationDir,
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

if ([string]::IsNullOrWhiteSpace($SdkDir)) {
    $SdkDir = $env:TFM2_MOD_SDK
}
if ([string]::IsNullOrWhiteSpace($DestinationDir)) {
    if ([string]::IsNullOrWhiteSpace($SdkDir)) {
        throw "Pass -DestinationDir or provide -SdkDir so the live mod directory can be inferred."
    }
    $resolvedSdk = (Resolve-Path -LiteralPath $SdkDir).Path
    $gameRoot = Split-Path -Parent $resolvedSdk
    $DestinationDir = Join-Path $gameRoot "mods\tfm2_better_mod_menu"
}

if (-not $SkipBuild) {
    & (Join-Path $root "build_local.ps1") -SdkDir $SdkDir
    if ($LASTEXITCODE -ne 0) {
        throw "Stable API build failed with exit code $LASTEXITCODE."
    }
}

$sourceDll = Join-Path $root "tfm2_better_mod_menu.dll"
$sourceBuildInfo = Join-Path $root "tfm2_better_mod_menu.build.json"
foreach ($required in @($sourceDll, $sourceBuildInfo)) {
    if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
        throw "Build output is missing: $required"
    }
}

$buildInfo = Get-Content -LiteralPath $sourceBuildInfo -Raw | ConvertFrom-Json
$sourceHash = (Get-FileHash -LiteralPath $sourceDll -Algorithm SHA256).Hash.ToLowerInvariant()
if ($sourceHash -ne $buildInfo.dll_sha256) {
    throw "The canonical DLL does not match its build manifest. Rebuild before installing."
}

$destination = [System.IO.Path]::GetFullPath($DestinationDir)
New-Item -ItemType Directory -Path $destination -Force | Out-Null

foreach ($name in @(
    "tfm2_better_mod_menu.dll",
    "tfm2_better_mod_menu.build.json",
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
    $source = Join-Path $root $name
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
        throw "Required runtime file is missing: $name"
    }
    Copy-Item -LiteralPath $source -Destination (Join-Path $destination $name) -Force
}

foreach ($relativeDirectory in @("ui\icons", "ui\layout", "third_party\licenses")) {
    $source = Join-Path $root $relativeDirectory
    $target = Join-Path $destination $relativeDirectory
    if (-not (Test-Path -LiteralPath $source -PathType Container)) {
        throw "Required runtime directory is missing: $relativeDirectory"
    }
    New-Item -ItemType Directory -Path $target -Force | Out-Null
    Copy-Item -Path (Join-Path $source "*") -Destination $target -Recurse -Force
}

& (Join-Path $PSScriptRoot "verify_install.ps1") -DestinationDir $destination
Write-Host "Better Mod Menu installed to: $destination"
