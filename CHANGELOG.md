# Changelog

All notable public changes to Better Mod Menu are documented here.

## [Unreleased]

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

[Unreleased]: https://github.com/MadManPetr1/tfm2-better-mod-menu/compare/v0.5.3...HEAD
[0.5.3]: https://github.com/MadManPetr1/tfm2-better-mod-menu/tree/v0.5.3
