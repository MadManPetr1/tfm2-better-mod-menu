# Changelog

All notable public changes to Better Mod Menu are documented here.

## [Unreleased]

### Changed

- Preserved the last `0.7.6` implementation separately from the in-progress
  `0.8.0` Stable API migration.
- Replaced the retired private-SDK build path with the Stable API SDK shipped by
  Teamfight Manager 2 `0.6.0+`.
- Added reproducible build manifests, staged installation, and source/DLL hash
  verification so local runtime artifacts can be traced to an exact revision.

### Compatibility

- The active development line now targets TFM2 `0.6.0+` and Stable ABI modules
  only. Restoring full `0.7.6` menu feature parity is tracked separately from
  this build-foundation work.

## [0.7.6] - 2026-09-02

### Changed

- Rebuilt the native DLL against the Teamfight Manager 2 `0.5.8` Mod SDK.
- Updated the runtime version gate and declared base range to
  `>=0.5.8, <0.5.9`.

### Compatibility

- Tested on Teamfight Manager 2 `0.5.8`; the existing menu, settings, restart,
  thumbnail, dependency and author-profile behavior is unchanged.

## [0.7.5] - 2026-08-27

### Fixed

- Made the ON and OFF halves of manifest-driven switches set explicit values
  instead of toggling the setting from either side.
- Prevented the native right-hand mod details panel from rendering underneath
  Better Mod Menu while its replacement panel is ready.
- Stopped competing preview refresh paths from alternating thumbnail fallback
  and dependency text, eliminating the selected-mod header flicker.

### Changed

- Cached static mod-row metadata and file-card paths instead of rebuilding them
  during routine UI updates.
- Reduced unchanged style, font, preview and settings writes so the menu dirties
  fewer UI nodes while idle.
- Kept the native panel available as a fallback if Better Mod Menu cannot finish
  preparing its own runtime surface.

### Compatibility

- Built and manually tested on Teamfight Manager 2 `0.5.7`, including Intro
  Skip settings, Overview/Settings switching, thumbnails and dependency text.

## [0.7.4] - 2026-08-26

### Changed

- Rebuilt the native DLL against the Teamfight Manager 2 `0.5.7` Mod SDK.
- Restricted the declared base range to `>=0.5.7, <0.5.8` because the native
  SDK libraries changed and older game versions have not been runtime-tested
  with this build.

### Compatibility

- Tested on Teamfight Manager 2 `0.5.7`.

## [0.7.3] - 2026-08-20

### Changed

- Rebuilt the native DLL against the Teamfight Manager 2 `0.5.6` Mod SDK.
- Restricted the declared base range to `>=0.5.6, <0.5.7` because the native
  SDK libraries changed and older game versions have not been runtime-tested
  with this build.

### Compatibility

- Tested on Teamfight Manager 2 `0.5.6`.

## [0.7.2] - 2026-08-12

### Changed

- Rebuilt the native DLL against the Teamfight Manager 2 `0.5.5` Mod SDK.
- Restricted the declared base range to `>=0.5.5, <0.5.6` because the native
  SDK libraries changed and older game versions have not been runtime-tested
  with this build.

### Compatibility

- Tested on Teamfight Manager 2 `0.5.5`.

## [0.7.1] - 2026-08-05

### Changed

- Rebuilt the native DLL against the Teamfight Manager 2 `0.5.4` Mod SDK.
- Extended the supported base range to `>=0.5.3, <0.5.5` without changing
  menu behavior or stored integration data.

### Compatibility

- Runtime-tested on Teamfight Manager 2 `0.5.4`.

## [0.7.0] - 2026-08-04

### Added

- Compact Enable All and Disable All actions using the game's native mod-toggle
  event path; Disable All always keeps Better Mod Menu enabled.
- Contained ten-row mod list with mouse-wheel scrolling, synchronized keyboard
  navigation, and automatic selected-row visibility.

### Changed

- Aligned the public runtime identity with the unified TFM2 mod template:
  `tfm2_better_mod_menu` for the mod ID, installed folder, crate, and DLL.
- Renamed release archives to the repository-aligned
  `tfm2-better-mod-menu-v<version>.zip` format.
- Consolidated manifest documentation into one modder guide and moved README
  screenshots under `assets/previews`.
- Replaced the README gallery with current Overview, Settings, Restart and
  Apply, and author-profile captures prepared for Steam's preview limit.
- Clarified that JSON-only Better Mod Menu integration does not impose GPL
  licensing or source-publication requirements on another mod.
- Refined mod-row interaction regions so hover, pointer feedback, and clicks
  track the visible controls more closely.

### Fixed

- Prevented long mod lists from extending behind the fixed footer and details
  panel.
- Prevented bulk actions from dropping back to the title screen partway through
  larger installed-mod sets.
- Kept keyboard navigation to one row per input without multi-row jumps.

## [0.6.0] - 2026-08-03

### Added

- Optional author profiles with a local icon, short bio, and validated GitHub,
  YouTube, and Discord contact fields.
- Public author-profile schema and copy-ready integration example.
- Compact settings categories for related manifest controls.
- New 256 px pixel-art project thumbnail and README showcase gallery.

### Changed

- Reworked the mod list, header, Overview, Settings, and author-card presentation
  for clearer hierarchy and better use of wide screens.
- Simplified player documentation and separated the five-minute modder path from
  the complete manifest reference.

### Fixed

- Kept profile popups above dynamically loaded overview artwork.
- Avoided repeated image replacement and unnecessary UI work after assets load.

### Compatibility

- Built and runtime-tested for Teamfight Manager 2 `0.5.3`.

## [0.5.3] - 2026-07-30

### Added

- Search, source filters, keyboard navigation, and native-styled mod toggles.
- Rich mod previews with dependency health, thumbnails, and optional banners.
- Optional manifest-driven toggles, choices, action buttons, and file cards.
- Clean restart-and-apply workflow for pending mod changes.
- Public manifest schema and integration guide for other mod authors.

### Safety and performance

- Reject manifest IDs and paths that could escape the declaring mod's storage.
- Require manifest IDs to match the installed mod identity.
- Cache dependency state instead of reading the mod configuration every frame.
- Apply native row layouts only when the mod menu opens or its row count changes.
- Avoid repeated label invalidation when text or fonts are unchanged.
- Guard cross-DLL UI runner casts with exact SDK type names.

### License

- Released under GPL-3.0-or-later with a narrow TFM2 linking exception.
- Required preservation of author/repository credit and reserved the original
  Better Mod Menu name and branding for the original project.

### Compatibility

- Built and runtime-tested for Teamfight Manager 2 `0.5.3`.

[Unreleased]: https://github.com/MadManPetr1/tfm2-better-mod-menu/compare/v0.7.5...HEAD
[0.7.5]: https://github.com/MadManPetr1/tfm2-better-mod-menu/compare/v0.7.4...v0.7.5
[0.7.4]: https://github.com/MadManPetr1/tfm2-better-mod-menu/compare/v0.7.3...v0.7.4
[0.7.3]: https://github.com/MadManPetr1/tfm2-better-mod-menu/compare/v0.7.2...v0.7.3
[0.7.2]: https://github.com/MadManPetr1/tfm2-better-mod-menu/compare/v0.7.1...v0.7.2
[0.7.1]: https://github.com/MadManPetr1/tfm2-better-mod-menu/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/MadManPetr1/tfm2-better-mod-menu/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/MadManPetr1/tfm2-better-mod-menu/compare/v0.5.3...v0.6.0
[0.5.3]: https://github.com/MadManPetr1/tfm2-better-mod-menu/tree/v0.5.3
