# SPDX-License-Identifier: GPL-3.0-or-later
# See LICENSE-EXCEPTION.md for the TFM2 linking exception and attribution terms.

param()

$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

foreach ($relativePath in @(
    "Cargo.toml",
    "Cargo.lock",
    "src/lib.rs",
    "mod.mod_info",
    "mod.override_info",
    "better_mod_menu.schema.json",
    "README.md",
    "CHANGELOG.md",
    "CONTRIBUTING.md",
    "SECURITY.md",
    "LICENSE",
    "LICENSE-EXCEPTION.md",
    "NOTICE.md",
    "MANIFEST.md",
    "MODDER_GUIDE.md",
    "THIRD_PARTY_NOTICES.md",
    "scripts/package_release.ps1",
    "ui/layout/better_mod_menu_runtime.ui",
    "ui/layout/mods_component/mod_file_cards_row_runtime.ui",
    "ui/layout/mods_component/mod_setting_category_runtime.ui",
    "ui/layout/mods_component/mod_setting_row_runtime.ui",
    "ui/layout/mods_component/mod_slot_runtime.ui",
    "ui/layout/mods_component/owned_mod_row_runtime.ui",
    "ui/icons/status-restart.png",
    "ui/icons/status-update.png",
    "ui/icons/status-warning.png"
)) {
    if (-not (Test-Path -LiteralPath (Join-Path $root $relativePath) -PathType Leaf)) {
        throw "Required file is missing: $relativePath"
    }
}

$modInfo = Get-Content -LiteralPath (Join-Path $root "mod.mod_info") -Raw | ConvertFrom-Json
$cargo = Get-Content -LiteralPath (Join-Path $root "Cargo.toml") -Raw
$cargoVersion = [regex]::Match($cargo, '(?m)^version\s*=\s*"([^"]+)"').Groups[1].Value
if ($cargoVersion -ne $modInfo.version) {
    throw "Version mismatch: Cargo.toml is $cargoVersion, mod.mod_info is $($modInfo.version)."
}
if ($modInfo.mod_id -ne "mod_menu") {
    throw "mod.mod_info must declare mod_id mod_menu."
}
$base = @($modInfo.dependencies | Where-Object { $_.mod_id -eq "base" })
if ($base.Count -ne 1 -or $base[0].version -ne ">=0.5.3, <0.5.4") {
    throw "Better Mod Menu must declare the tested 0.5.3 base range."
}
if ($cargo -notmatch '(?m)^license\s*=\s*"GPL-3\.0-or-later"') {
    throw "Cargo.toml must declare GPL-3.0-or-later."
}

Get-Content -LiteralPath (Join-Path $root "better_mod_menu.schema.json") -Raw |
    ConvertFrom-Json | Out-Null

Push-Location $root
try {
    cargo fmt --check
    if ($LASTEXITCODE -ne 0) {
        throw "cargo fmt --check failed."
    }
}
finally {
    Pop-Location
}

Write-Host "Repository validation passed for Better Mod Menu v$($modInfo.version)."
