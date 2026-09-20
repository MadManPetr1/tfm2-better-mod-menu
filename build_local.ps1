# SPDX-License-Identifier: GPL-3.0-or-later
# See LICENSE-EXCEPTION.md for the TFM2 linking exception and attribution terms.

param(
    [string]$SdkDir = $env:TFM2_MOD_SDK_STABLE
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($SdkDir)) {
    $SdkDir = $env:TFM2_MOD_SDK
}
if ([string]::IsNullOrWhiteSpace($SdkDir)) {
    throw "Pass -SdkDir <path-to-mod-sdk-stable> or set TFM2_MOD_SDK_STABLE."
}

$root = (Resolve-Path -LiteralPath $PSScriptRoot).Path
$sdk = (Resolve-Path -LiteralPath $SdkDir).Path
$sdkBaseVersionFile = Join-Path $sdk "base_version.txt"
$sdkCrate = Join-Path $sdk "mod-api-stable"
$sdkCrateManifest = Join-Path $sdkCrate "Cargo.toml"
$sdkContract = Join-Path $sdkCrate "contract\abi_v1.txt"

foreach ($required in @($sdkBaseVersionFile, $sdkCrateManifest, $sdkContract)) {
    if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
        throw "The Stable Mod SDK is incomplete; missing $required"
    }
}

$baseVersionText = (Get-Content -LiteralPath $sdkBaseVersionFile -Raw).Trim()
try {
    $baseVersion = [version]$baseVersionText
}
catch {
    throw "The Stable Mod SDK base_version.txt is invalid: $baseVersionText"
}
if ($baseVersion -lt [version]"0.6.0") {
    throw "Better Mod Menu 0.8.0 requires the Stable Mod SDK from TFM2 0.6.0 or newer; found $baseVersionText."
}

$contractText = Get-Content -LiteralPath $sdkContract -Raw
$abiMatch = [regex]::Match($contractText, '(?m)^abi_level\s*=\s*(\d+)\s*$')
if (-not $abiMatch.Success) {
    throw "Could not read abi_level from $sdkContract"
}
$abiLevel = [int]$abiMatch.Groups[1].Value

$vendorParent = Join-Path $root "vendor"
$vendorTarget = Join-Path $vendorParent "mod-api-stable"
$expectedVendorTarget = [System.IO.Path]::GetFullPath($vendorTarget)
New-Item -ItemType Directory -Path $vendorParent -Force | Out-Null
if (Test-Path -LiteralPath $vendorTarget) {
    $resolvedVendorTarget = (Resolve-Path -LiteralPath $vendorTarget).Path
    if (-not $resolvedVendorTarget.Equals($expectedVendorTarget, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to replace an unexpected vendor path: $resolvedVendorTarget"
    }
    Remove-Item -LiteralPath $resolvedVendorTarget -Recurse -Force
}
Copy-Item -LiteralPath $sdkCrate -Destination $vendorTarget -Recurse

$manifestPath = Join-Path $root "Cargo.toml"
$cargoText = Get-Content -LiteralPath $manifestPath -Raw
$versionMatch = [regex]::Match($cargoText, '(?m)^version\s*=\s*"([^"]+)"')
if (-not $versionMatch.Success) {
    throw "Could not read the package version from Cargo.toml."
}
$modVersion = $versionMatch.Groups[1].Value

$sourceCommit = (& git -C $root rev-parse --short=12 HEAD | Select-Object -First 1).Trim()
if ([string]::IsNullOrWhiteSpace($sourceCommit)) {
    throw "Could not determine the source revision."
}
$dirtyOutput = @(& git -C $root status --porcelain=v1 --untracked-files=all)
$sourceDirty = $dirtyOutput.Count -gt 0
$sourceDiff = @(& git -C $root diff --binary --no-ext-diff HEAD -- .) -join "`n"
$untrackedSignatures = @(
    & git -C $root ls-files --others --exclude-standard |
        Sort-Object |
        ForEach-Object {
            $untrackedPath = $_
            $untrackedHash = (Get-FileHash -LiteralPath (Join-Path $root $untrackedPath) -Algorithm SHA256).Hash.ToLowerInvariant()
            "$untrackedPath`:$untrackedHash"
        }
)
$sourceMaterial = $sourceDiff + "`n--untracked--`n" + ($untrackedSignatures -join "`n")
$sourceDiffBytes = [System.Text.UTF8Encoding]::new($false).GetBytes($sourceMaterial)
$sourceDiffHash = [System.BitConverter]::ToString(
    [System.Security.Cryptography.SHA256]::HashData($sourceDiffBytes)
).Replace("-", "").ToLowerInvariant()
$sourceFingerprint = if ($sourceDirty) { "$sourceCommit`:$sourceDiffHash" } else { $sourceCommit }
$buildRevision = if ($sourceDirty) {
    "$sourceCommit+dirty.$($sourceDiffHash.Substring(0, 12))"
}
else {
    $sourceCommit
}
$buildTimestamp = [DateTime]::UtcNow.ToString("yyyy-MM-ddTHH:mm:ssZ")

$env:TFM2_BMM_BUILD_REVISION = $buildRevision
$env:TFM2_BMM_BUILD_TIMESTAMP = $buildTimestamp

cargo build --release --locked --manifest-path $manifestPath
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

$generatedDll = Join-Path $root "target\release\tfm2_better_mod_menu.dll"
$outputDll = Join-Path $root "tfm2_better_mod_menu.dll"
$outputManifest = Join-Path $root "tfm2_better_mod_menu.build.json"
if (-not (Test-Path -LiteralPath $generatedDll -PathType Leaf)) {
    throw "Cargo build succeeded, but the generated DLL is missing: $generatedDll"
}
Copy-Item -LiteralPath $generatedDll -Destination $outputDll -Force
$dllHash = (Get-FileHash -LiteralPath $outputDll -Algorithm SHA256).Hash.ToLowerInvariant()

$buildInfo = [ordered]@{
    schema_version = 1
    mod_id = "tfm2_better_mod_menu"
    version = $modVersion
    minimum_game_version = "0.6.0"
    sdk_base_version = $baseVersionText
    sdk_abi_level = $abiLevel
    source_commit = $sourceCommit
    source_dirty = $sourceDirty
    source_fingerprint = $sourceFingerprint
    source_revision = $buildRevision
    built_at_utc = $buildTimestamp
    dll_file = "tfm2_better_mod_menu.dll"
    dll_sha256 = $dllHash
}
$buildInfoJson = $buildInfo | ConvertTo-Json -Depth 4
[System.IO.File]::WriteAllText(
    $outputManifest,
    $buildInfoJson + [Environment]::NewLine,
    [System.Text.UTF8Encoding]::new($false)
)

Write-Host "Stable API DLL ready: $outputDll"
Write-Host "Build identity: $buildRevision | SDK $baseVersionText | ABI $abiLevel | SHA256 $dllHash"
