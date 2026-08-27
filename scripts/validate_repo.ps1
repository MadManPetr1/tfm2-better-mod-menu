# SPDX-License-Identifier: GPL-3.0-or-later
# See LICENSE-EXCEPTION.md for the TFM2 linking exception and attribution terms.

param()

$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Add-Type -AssemblyName System.Drawing

foreach ($relativePath in @(
    "Cargo.toml",
    "Cargo.lock",
    "src/lib.rs",
    "mod.mod_info",
    "mod.override_info",
    "better_mod_menu.schema.json",
    "better_mod_menu.json.example",
    "better_mod_menu_profile.json",
    "better_mod_menu_profile.json.example",
    "better_mod_menu_profile.schema.json",
    "assets/thumbnail-master.png",
    "assets/previews/overview.png",
    "assets/previews/settings.png",
    "assets/previews/restart-and-apply.png",
    "assets/previews/author-profile.png",
    "profile_icon.png",
    "thumbnail.png",
    "README.md",
    "workshop_description.txt",
    "CHANGELOG.md",
    "CONTRIBUTING.md",
    "LICENSE",
    "LICENSE-EXCEPTION.md",
    "NOTICE.md",
    "MODDER_GUIDE.md",
    "THIRD_PARTY_NOTICES.md",
    "scripts/package_release.ps1",
    "ui/layout/better_mod_menu_runtime.ui",
    "ui/layout/mods_component/mod_file_cards_row_runtime.ui",
    "ui/layout/mods_component/mod_setting_category_runtime.ui",
    "ui/layout/mods_component/mod_setting_row_runtime.ui",
    "ui/layout/mods_component/mod_slot_runtime.ui",
    "ui/layout/mods_component/bmm_mod_row_runtime.ui",
    "ui/icons/author.png",
    "ui/icons/profile-discord.png",
    "ui/icons/profile-github.png",
    "ui/icons/profile-youtube.png",
    "ui/icons/status-restart.png",
    "ui/icons/status-update.png",
    "ui/icons/status-warning.png"
)) {
    if (-not (Test-Path -LiteralPath (Join-Path $root $relativePath) -PathType Leaf)) {
        throw "Required file is missing: $relativePath"
    }
}

foreach ($relativePath in @(
    "src/dev_inspector.rs",
    "src/ui_source.rs",
    "ui/layout/dev/bmm_ui_inspector_runtime.ui",
    "better_mod_menu.dev.json",
    "better_mod_menu.dev.json.example"
)) {
    if (Test-Path -LiteralPath (Join-Path $root $relativePath)) {
        throw "Developer-only file must not be present in the public repository: $relativePath"
    }
}

$modInfo = Get-Content -LiteralPath (Join-Path $root "mod.mod_info") -Raw | ConvertFrom-Json
$cargo = Get-Content -LiteralPath (Join-Path $root "Cargo.toml") -Raw
$cargoLock = Get-Content -LiteralPath (Join-Path $root "Cargo.lock") -Raw
$workshop = Get-Content -LiteralPath (Join-Path $root "workshop_description.txt") -Raw
$cargoVersion = [regex]::Match($cargo, '(?m)^version\s*=\s*"([^"]+)"').Groups[1].Value
$lockVersion = [regex]::Match(
    $cargoLock,
    '(?ms)\[\[package\]\]\s+name\s*=\s*"tfm2_better_mod_menu"\s+version\s*=\s*"([^"]+)"'
).Groups[1].Value
if ($cargoVersion -ne $modInfo.version -or $lockVersion -ne $modInfo.version) {
    throw "Version mismatch between Cargo.toml, Cargo.lock, and mod.mod_info."
}
if ($modInfo.mod_id -ne "tfm2_better_mod_menu") {
    throw "mod.mod_info must declare mod_id tfm2_better_mod_menu."
}
if ($cargo -notmatch '(?m)^name\s*=\s*"tfm2_better_mod_menu"') {
    throw "Cargo.toml package name must be tfm2_better_mod_menu."
}
$base = @($modInfo.dependencies | Where-Object { $_.mod_id -eq "base" })
if ($base.Count -ne 1 -or $base[0].version -ne ">=0.5.7, <0.5.8") {
    throw "Better Mod Menu must declare the tested 0.5.7 base range."
}
if ($cargo -notmatch '(?m)^license\s*=\s*"GPL-3\.0-or-later"') {
    throw "Cargo.toml must declare GPL-3.0-or-later."
}
if ($workshop -notmatch [regex]::Escape("[code]tfm2_better_mod_menu.dll[/code]")) {
    throw "Workshop description must name tfm2_better_mod_menu.dll."
}
if ($workshop -notmatch [regex]::Escape("[b]Current version:[/b] v$($modInfo.version)")) {
    throw "Workshop description version does not match mod.mod_info."
}
if ($workshop -notmatch [regex]::Escape("[url=https://github.com/MadManPetr1/tfm2-better-mod-menu]Source code on GitHub[/url]")) {
    throw "Workshop description must link to the public source repository."
}
if ($workshop -notmatch '\[b\]Tested with:\[/b\] TFM2 0\.5\.7') {
    throw "Workshop Tested with line must match the supported base range."
}
if ($workshop -notmatch '(?m)^\[b\]Last tested:\[/b\] \d{2}/\d{2}/\d{4}$') {
    throw "Workshop Last tested must use DD/MM/YYYY."
}
$featuresStart = $workshop.IndexOf("[h2]Features[/h2]", [StringComparison]::Ordinal)
$featuresEnd = $workshop.IndexOf("[h2]Compatibility[/h2]", [StringComparison]::Ordinal)
if ($featuresStart -lt 0 -or $featuresEnd -le $featuresStart) {
    throw "Workshop Features and Compatibility sections are missing or out of order."
}
$featureCount = [regex]::Matches(
    $workshop.Substring($featuresStart, $featuresEnd - $featuresStart),
    '(?m)^\[\*\]'
).Count
if ($featureCount -lt 1 -or $featureCount -gt 6) {
    throw "Workshop Features must contain between one and six entries."
}

Get-Content -LiteralPath (Join-Path $root "better_mod_menu.schema.json") -Raw |
    ConvertFrom-Json | Out-Null
Get-Content -LiteralPath (Join-Path $root "better_mod_menu_profile.schema.json") -Raw |
    ConvertFrom-Json | Out-Null
Get-Content -LiteralPath (Join-Path $root "better_mod_menu_profile.json") -Raw |
    ConvertFrom-Json | Out-Null

$publicSchemaRoot = "https://raw.githubusercontent.com/MadManPetr1/tfm2-better-mod-menu/main"
foreach ($entry in @(
    @{ File = "better_mod_menu.json.example"; Schema = "$publicSchemaRoot/better_mod_menu.schema.json" },
    @{ File = "better_mod_menu_profile.json"; Schema = "$publicSchemaRoot/better_mod_menu_profile.schema.json" },
    @{ File = "better_mod_menu_profile.json.example"; Schema = "$publicSchemaRoot/better_mod_menu_profile.schema.json" }
)) {
    $json = Get-Content -LiteralPath (Join-Path $root $entry.File) -Raw | ConvertFrom-Json
    if ($json.'$schema' -ne $entry.Schema) {
        throw "$($entry.File) must reference the public GitHub schema."
    }
}

$profile = Get-Content -LiteralPath (Join-Path $root "better_mod_menu_profile.json") -Raw |
    ConvertFrom-Json
if ($profile.schema_version -ne 1 -or $profile.profile_icon -ne "profile_icon.png") {
    throw "better_mod_menu_profile.json must use schema version 1 and profile_icon.png."
}

$thumbnail = [System.Drawing.Image]::FromFile((Join-Path $root "thumbnail.png"))
try {
    if ($thumbnail.Width -ne 256 -or $thumbnail.Height -ne 256) {
        throw "thumbnail.png must be 256x256."
    }
}
finally {
    $thumbnail.Dispose()
}

$thumbnailMaster = [System.Drawing.Image]::FromFile((Join-Path $root "assets/thumbnail-master.png"))
try {
    if ($thumbnailMaster.Width -ne 128 -or $thumbnailMaster.Height -ne 128) {
        throw "assets/thumbnail-master.png must be 128x128."
    }
}
finally {
    $thumbnailMaster.Dispose()
}

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
