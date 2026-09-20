# SPDX-License-Identifier: GPL-3.0-or-later
# See LICENSE-EXCEPTION.md for the TFM2 linking exception and attribution terms.

param(
    [Parameter(Mandatory = $true)]
    [string]$DestinationDir
)

$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$destination = (Resolve-Path -LiteralPath $DestinationDir).Path
$sourceDll = Join-Path $root "tfm2_better_mod_menu.dll"
$sourceBuildInfoPath = Join-Path $root "tfm2_better_mod_menu.build.json"
$installedDll = Join-Path $destination "tfm2_better_mod_menu.dll"
$installedBuildInfoPath = Join-Path $destination "tfm2_better_mod_menu.build.json"
$installedModInfoPath = Join-Path $destination "mod.mod_info"

foreach ($required in @(
    $sourceDll,
    $sourceBuildInfoPath,
    $installedDll,
    $installedBuildInfoPath,
    $installedModInfoPath
)) {
    if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
        throw "Install verification is missing a required file: $required"
    }
}

$sourceBuildInfo = Get-Content -LiteralPath $sourceBuildInfoPath -Raw | ConvertFrom-Json
$installedBuildInfo = Get-Content -LiteralPath $installedBuildInfoPath -Raw | ConvertFrom-Json
$installedModInfo = Get-Content -LiteralPath $installedModInfoPath -Raw | ConvertFrom-Json
$sourceHash = (Get-FileHash -LiteralPath $sourceDll -Algorithm SHA256).Hash.ToLowerInvariant()
$installedHash = (Get-FileHash -LiteralPath $installedDll -Algorithm SHA256).Hash.ToLowerInvariant()

if ($sourceHash -ne $sourceBuildInfo.dll_sha256) {
    throw "The canonical DLL hash does not match the source build manifest."
}
if ($installedHash -ne $sourceHash -or $installedHash -ne $installedBuildInfo.dll_sha256) {
    throw "The installed DLL is not the canonical build artifact."
}
if ($installedBuildInfo.source_revision -ne $sourceBuildInfo.source_revision) {
    throw "The installed and source build manifests name different revisions."
}
if ($installedBuildInfo.version -ne $installedModInfo.version) {
    throw "The installed build manifest and mod.mod_info versions do not match."
}
if ($installedBuildInfo.minimum_game_version -ne "0.6.0") {
    throw "The installed build does not declare the TFM2 0.6.0 Stable API baseline."
}

$currentCommit = (& git -C $root rev-parse --short=12 HEAD | Select-Object -First 1).Trim()
$currentDirtyOutput = @(& git -C $root status --porcelain=v1 --untracked-files=all)
$currentSourceDirty = $currentDirtyOutput.Count -gt 0
$currentDiff = @(& git -C $root diff --binary --no-ext-diff HEAD -- .) -join "`n"
$currentUntrackedSignatures = @(
    & git -C $root ls-files --others --exclude-standard |
        Sort-Object |
        ForEach-Object {
            $untrackedPath = $_
            $untrackedHash = (Get-FileHash -LiteralPath (Join-Path $root $untrackedPath) -Algorithm SHA256).Hash.ToLowerInvariant()
            "$untrackedPath`:$untrackedHash"
        }
)
$currentSourceMaterial = $currentDiff + "`n--untracked--`n" + ($currentUntrackedSignatures -join "`n")
$currentDiffBytes = [System.Text.UTF8Encoding]::new($false).GetBytes($currentSourceMaterial)
$currentDiffHash = [System.BitConverter]::ToString(
    [System.Security.Cryptography.SHA256]::HashData($currentDiffBytes)
).Replace("-", "").ToLowerInvariant()
$currentFingerprint = if ($currentSourceDirty) {
    "$currentCommit`:$currentDiffHash"
}
else {
    $currentCommit
}
if ($sourceBuildInfo.source_fingerprint -ne $currentFingerprint) {
    throw "The build fingerprint does not match the current source tree. Rebuild before installing."
}

$binaryText = [System.Text.Encoding]::ASCII.GetString([System.IO.File]::ReadAllBytes($installedDll))
if (-not $binaryText.Contains([string]$installedBuildInfo.source_revision)) {
    throw "The installed DLL does not contain the source revision recorded by its build manifest."
}

Write-Host "Install verified: v$($installedBuildInfo.version) | $($installedBuildInfo.source_revision) | ABI $($installedBuildInfo.sdk_abi_level) | SHA256 $installedHash"
