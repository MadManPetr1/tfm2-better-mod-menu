# SPDX-License-Identifier: GPL-3.0-or-later
# See LICENSE-EXCEPTION.md for the TFM2 linking exception and attribution terms.

param(
    [Parameter(Mandatory = $true)]
    [string]$SdkDir
)

$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$resolvedSdk = (Resolve-Path -LiteralPath $SdkDir).Path
$tempRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
$stageRoot = Join-Path $tempRoot "tfm2-bmm-foundation-$PID"

try {
    & (Join-Path $PSScriptRoot "validate_repo.ps1")
    & (Join-Path $root "build_local.ps1") -SdkDir $resolvedSdk
    & (Join-Path $PSScriptRoot "install_local.ps1") `
        -SdkDir $resolvedSdk `
        -DestinationDir $stageRoot `
        -SkipBuild
    & (Join-Path $PSScriptRoot "verify_install.ps1") -DestinationDir $stageRoot

    $canonicalDll = Join-Path $root "tfm2_better_mod_menu.dll"
    $stagedDll = Join-Path $stageRoot "tfm2_better_mod_menu.dll"
    $canonicalHash = (Get-FileHash -LiteralPath $canonicalDll -Algorithm SHA256).Hash
    $stagedHash = (Get-FileHash -LiteralPath $stagedDll -Algorithm SHA256).Hash
    if ($canonicalHash -ne $stagedHash) {
        throw "The staged DLL does not match the canonical build artifact."
    }

    Write-Host "Stable API foundation test passed."
}
finally {
    if (Test-Path -LiteralPath $stageRoot) {
        $resolvedStage = (Resolve-Path -LiteralPath $stageRoot).Path
        if (-not $resolvedStage.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing to remove a staging directory outside the system temp folder: $resolvedStage"
        }
        Remove-Item -LiteralPath $resolvedStage -Recurse -Force
    }
}
