# Better Mod Menu Phase 3: Generic Data Core

Date: 2026-09-20

Status: Proposed for implementation after user review

## Purpose

Phase 3 replaces the partial Stable API rewrite's scattered filesystem and
hardcoded integration logic with one typed data and command layer for Teamfight
Manager 2 `0.6.0+`. It restores the non-visual foundation needed for the
v0.7.6 feature set without porting the retired private-API implementation.

This phase is intentionally not a UI redesign. The current Stable API menu
continues to render while the new core becomes its source of installed-mod,
metadata, dependency, profile, asset, and enabled-state data. Generic settings
rows and the hitbox/layout overhaul remain Phase 4 work.

## Goals

- Discover Local and Workshop mods through one deterministic catalog.
- Treat `mod.mod_info` as the required baseline metadata source.
- Parse the two optional Better Mod Menu files without affecting normal mod
  loading when they are missing or invalid:
  - `better_mod_menu.json`
  - `better_mod_menu_profile.json`
- Produce one selected-mod view model containing all data a later UI renderer
  needs.
- Evaluate declared dependencies and turn them into readable health results.
- Load and mutate manifest-driven settings without discarding unknown JSON
  fields.
- Write settings, action requests, and enabled-mod configuration through one
  safe atomic JSON path.
- Keep Better Mod Menu enabled during bulk-disable commands.
- Report non-fatal integration problems through structured issues and the
  Stable API log.
- Cover the core with filesystem-fixture and pure unit tests.

## Non-goals

- No visual redesign or spacing changes.
- No new row, button, profile-card, tooltip, or settings rendering.
- No hitbox migration; that belongs to Phase 4.
- No sorting, profiles/presets, diagnostics screen, update service, network
  requests, dependency auto-enabling, or automatic dependency installation.
- No support for the TFM2 `0.5.x` private Mod SDK.
- No additional manifest or profile schema version.
- No release packaging or public documentation reconciliation.

## Current constraints

- The runtime targets the Stable API shipped with TFM2 `0.6.0+` and currently
  builds against ABI level 9.
- The active v0.8 source scans both install roots but holds only a minimal
  `ModEntry` and hardcodes Intro Skip settings behavior.
- The preserved v0.7.6 source is a behavioral reference, not code to copy
  wholesale.
- The public schemas in the repository remain authoritative for version 1 of
  both optional JSON files.
- Missing or malformed optional files must never remove a mod from the catalog.

## Architecture

Phase 3 introduces six focused modules while leaving Stable API registration
and UI orchestration in `src/lib.rs`:

### `src/model.rs`

Owns data types only:

- `ModSource`: `Local` or `Workshop`.
- `ModIdentity`: mod id, name, author, version, description, dependencies.
- `ModAssets`: optional thumbnail, banner, and profile-icon asset references.
- `AuthorProfile`: validated display name, bio, provider links/contact.
- `IntegrationManifest` and typed `IntegrationControl` variants.
- `DependencyHealth` and individual `DependencyIssue` values.
- `ModRecord`: the complete catalog entry.
- `SelectedModView`: display-ready data derived from a `ModRecord`.
- `CatalogIssue`: file, optional mod id, and concise non-fatal message.
- `StatusKind`: `None`, `RestartRequired`, `Warning`, and reserved `Update`,
  ordered as `Update > Warning > RestartRequired > None`.

The types do not depend on `StableClient` or UI paths.

### `src/catalog.rs`

Builds a `ModCatalog` from explicit roots and enabled ids.

Production roots are:

- `<game>/mods` for Local mods.
- `<steamapps>/workshop/content/3009300` for Workshop mods.

Tests pass temporary roots directly. Discovery supports the existing one-level
nested Workshop layout. Invalid `mod.mod_info` files are skipped with a
`CatalogIssue`; invalid optional Better Mod Menu files leave the baseline
`ModRecord` intact.

Entries are keyed by `mod_id`. Local wins when the same mod id exists in both
roots, matching the active v0.8 scan order and avoiding two UI rows that mutate
the same enabled id. A duplicate produces a non-fatal issue naming both roots.
The final catalog is sorted case-insensitively by display name, then mod id for
stable ordering.

The catalog stores the game version supplied by `StableHost::game_version()`;
dependency evaluation must not use a hardcoded TFM2 version.

### `src/integration.rs`

Parses and validates the two optional version-1 JSON formats.

For `better_mod_menu.json`:

- `schema_version` must be `1`.
- `mod_id` must be one safe path component and must match `mod.mod_info`.
- One to seven controls are accepted.
- Supported controls remain `toggle`, `choice`, `button`, and `file_cards`.
- Display overrides replace the corresponding baseline field individually;
  they do not create overlapping copies of title, author, version, or summary.
- Unsupported storage values, unsafe relative paths, empty keys/actions, empty
  choices, duplicate setting keys, and invalid file-card limits reject the
  optional manifest only.
- Unknown top-level or control fields follow the existing public schema and
  reject the optional manifest with a non-fatal issue.

For `better_mod_menu_profile.json`:

- `schema_version` must be `1`.
- Display name falls back to `mod.mod_info.author`.
- Bio is plain text and limited to 240 Unicode scalar values.
- Profile icon is limited to `profile_icon.png` or `profile_icon.jpg` and must
  exist in the owning mod root before an asset reference is exposed.
- GitHub and YouTube accept validated usernames/handles and are converted to
  canonical HTTPS provider URLs.
- Discord accepts only a validated modern username and remains a copyable
  contact value, not a URL.
- Arbitrary URLs, additional providers, avatars fetched from the network, and
  identity-verification claims are rejected.

Thumbnail and banner lookup is independent of enabled state. Candidates remain
`thumbnail.png`, `assets/thumbnail.png`, `banner.png`, and
`assets/banner.png`. The core exposes asset descriptors; Stable API property
updates remain the renderer's responsibility.

### `src/dependencies.rs`

Owns numeric version comparison, requirement parsing, friendly text, and health
evaluation for dependencies declared only through `mod.mod_info`.

Supported requirement clauses are `=`, `>`, `>=`, `<`, and `<=`, joined by
commas. Missing or malformed requirements do not panic; they generate a
dependency issue with the original requirement.

Examples:

- `base >=0.5.2, <0.5.4` becomes
  `TFM2 version >= 0.5.2 and < 0.5.4` in the internal plain-text form used by
  the game font.
- Another mod dependency becomes `Requires <display name> <requirement>`.

Health checks distinguish:

- Missing dependency.
- Installed but disabled dependency.
- Installed incompatible dependency version.
- Healthy dependency.

No dependency is automatically enabled or installed.

### `src/settings.rs`

Owns settings storage, commands, file-card discovery, and action requests.

Storage roots remain:

- `storage: "mod"`: the owning mod root.
- `storage: "game_data"`:
  `%APPDATA%/TeamSamoyed/TeamfightManager2/data/<mod_id>`.

Every declared file or directory is resolved as a relative path below its
allowed root. Absolute paths, prefixes, root components, and `..` are rejected.

`SettingsDocument` loads an object or starts as an empty object. Applying a
toggle or choice changes only its declared key and preserves all unknown keys.
Missing values use manifest defaults for display without writing until the user
actually changes the control.

Commands are typed:

- `SetToggle { key, value }`.
- `SelectChoice { key, option_index }`.
- `RequestAction { action }`.
- `RequestFileAction { action, index }`.

Action requests retain the existing interoperable payloads:

```json
{ "action": "action_name" }
```

and:

```json
{ "action": "action_name", "index": 0 }
```

File-card discovery is restricted to the game-data root, returns regular files
only, applies the declared filename and extension filters, sorts newest first,
and clamps the result to five entries.

### `src/io.rs`

Provides the single JSON read/write boundary for settings, actions, and
`config/game/mods.json`.

Writes use a sibling temporary file, flush it, and atomically replace the
destination on Windows with replacement semantics. The implementation must not
rely on `std::fs::rename` overwriting an existing Windows file. A failed replace
leaves the previous destination intact and removes the temporary file when
possible.

The enabled-mod document mutation preserves every field other than
`enabled_mods`, skips ids already in the requested state, and never removes
`tfm2_better_mod_menu` from the enabled set during a bulk-disable command.

The module returns descriptive errors; callers decide whether to log them or
show a later UI status.

## Data flow

At Stable API initialization:

1. Read the game version from `StableHost`.
2. Locate the game root and both mod roots.
3. Read `config/game/mods.json` once for enabled ids.
4. Build `ModCatalog` from required `mod.mod_info` files.
5. Enrich each valid record with optional integration/profile data and local
   asset descriptors.
6. Evaluate dependency health using the completed catalog and enabled set.
7. Log each unique `CatalogIssue` once.
8. Store the catalog in `SharedState` for the current renderer.

The current list/search/filter/detail renderer reads `ModRecord` and
`SelectedModView` instead of the old minimal `ModEntry`. Existing fixed Intro
Skip settings rows remain operational during this phase; they are not expanded
or redesigned. Phase 4 replaces that renderer with generic control rows and
removes the remaining Intro Skip-specific UI functions.

When a command is invoked by existing or later UI code:

1. Resolve the selected record and validated integration entry.
2. Validate the command against the declared control.
3. Load the latest destination object.
4. Apply only the requested change.
5. Write through the atomic JSON boundary.
6. Return a typed success or descriptive failure result.

## Error handling

- Required metadata failure: skip that directory and record an issue.
- Optional JSON failure: retain the baseline mod and ignore only the invalid
  optional feature.
- Invalid profile field: ignore the profile file as a unit so the card never
  mixes trusted and unvalidated values.
- Unsafe declared path: reject the optional manifest as a unit.
- Settings file missing: treat as an empty object.
- Settings file containing valid non-object JSON: report an error and do not
  overwrite it.
- JSON write failure: leave the previous file intact and return the error.
- Dependency parse failure: report a warning issue; never panic or silently
  claim the dependency is healthy.

No integration error may prevent the menu from opening or a normal
`mod.mod_info` entry from appearing.

## Testing

Tests use temporary directory fixtures and do not touch the live game install.
Coverage must include:

- Local and Workshop discovery, including one nested Workshop directory.
- Local precedence and duplicate-id reporting.
- Baseline mods with neither optional JSON file.
- Valid and invalid manifest/profile files.
- Per-field display override precedence.
- Profile fallback, bio length, icon filename/existence, provider validation,
  canonical URLs, and Discord contact validation.
- Thumbnail/banner discovery for disabled mods.
- Version comparisons and multi-clause requirements.
- Healthy, missing, disabled, incompatible, and malformed dependencies.
- Safe and unsafe relative paths.
- Toggle and choice writes that preserve unknown settings keys.
- Button and file-card action payloads.
- File-card sorting/filtering/clamping.
- Atomic replacement over an existing JSON file on Windows.
- Enabled-mod mutation preserving unrelated configuration fields.
- Bulk disable preserving Better Mod Menu.
- Catalog issues remaining non-fatal.

Existing search/filter and build-identity tests remain green. The phase ends
with `cargo fmt --check`, `cargo test --locked`, `cargo clippy --locked
--all-targets -- -D warnings`, foundation validation, a Stable SDK release
build, staged installation verification, and a short live smoke test of the
unchanged current menu surface.

## Acceptance criteria

- `src/lib.rs` no longer owns mod discovery, optional JSON parsing, dependency
  evaluation, settings persistence, or enabled-mod JSON mutation.
- All installed mods with valid `mod.mod_info` appear in the catalog regardless
  of optional integration files.
- Invalid optional files produce one non-fatal issue and do not remove the mod.
- The core exposes complete data for future Overview, Settings, dependency,
  profile, status, thumbnail, and banner rendering.
- The current menu remains operational with no intended visual changes.
- No TFM2 `0.5.x` SDK code or compatibility branch is introduced.
- The live DLL can still be traced to a clean source revision through the Phase
  2 build manifest.

## Deferred to Phase 4

- Generic creation and rendering of manifest controls.
- Removal of the fixed Intro Skip settings-row renderer.
- Direct visible-control event binding and removal of overlay hitboxes.
- Native scroll-view integration and synchronized keyboard scrolling.
- Profile-card hover/pin interaction, provider clicks, and Discord copy UI.
- Status tooltips and row icon presentation.
- Full restart/apply and bulk-action interaction parity.
