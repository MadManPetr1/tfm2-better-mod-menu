# Better Mod Menu Phase 3 Data Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Better Mod Menu's scattered Stable API filesystem logic with a tested generic catalog, integration, dependency, settings, and atomic JSON command layer while keeping the current menu surface operational.

**Architecture:** Introduce focused Rust modules beneath the existing Stable API entrypoint. Pure model, integration, dependency, settings, catalog, and JSON I/O code remains independent of UI paths; `src/lib.rs` becomes the adapter that logs catalog issues, stores `ModRecord` values, renders the existing surface, and delegates mutations.

**Tech Stack:** Rust 2021, `mod-api-stable` ABI 9, `serde`, `serde_json`, Windows API FFI from the standard library, PowerShell build/install verification.

**Spec:** `docs/superpowers/specs/2026-09-20-bmm-phase-3-data-core-design.md`

## Global Constraints

- Target only the Stable API shipped with Teamfight Manager 2 `0.6.0+`; do not add a TFM2 `0.5.x` compatibility path.
- Keep `mod.mod_info` as the required baseline metadata source and keep both Better Mod Menu JSON files optional.
- Missing or invalid optional JSON must not remove an otherwise valid mod from the catalog.
- Support only schema version `1` and the four existing control types: `toggle`, `choice`, `button`, and `file_cards`.
- Keep profile providers limited to GitHub, YouTube, and copy-only Discord; do not make network requests.
- Preserve unknown settings and `mods.json` fields on every mutation.
- Reject absolute, prefixed, rooted, or parent-traversing declared paths.
- Bulk disable must preserve `tfm2_better_mod_menu`.
- Keep Phase 3 visually neutral; generic control rendering and hitbox changes remain Phase 4 work.
- Add no third-party dependency unless the existing standard library and Stable API cannot implement the requirement.

## Review Focus

- A corrupt Local duplicate must not allow a Workshop duplicate with the same id to disappear silently; Task 6 tests required-metadata failure and duplicate precedence together.
- A path using mixed separators or a Windows drive/prefix must remain inside its allowed root; Task 5 tests `..`, rooted, drive-prefixed, and normal nested paths.
- An existing settings file containing valid non-object JSON must never be replaced with an object; Task 5 tests refusal and unchanged file bytes.
- A failed Windows destination replacement must preserve the old JSON and clean the sibling temporary file when possible; Task 4 tests successful replacement and isolates the replacement function for failure-path coverage.
- A malformed dependency clause must produce a warning instead of being treated as compatible; Task 3 includes malformed single- and multi-clause tests.

---

### Task 1: Domain Model and Status Priority

**Files:**
- Create: `src/model.rs`
- Modify: `src/lib.rs`
- Test: `src/model.rs` unit tests

**Interfaces:**
- Consumes: `serde_json::Value`, `std::path::PathBuf`.
- Produces: `ModSource`, `ModDependency`, `ModIdentity`, `DisplayMetadata`, `ModAssets`, `AuthorProfile`, `StorageKind`, `ChoiceOption`, `IntegrationControl`, `IntegrationManifest`, `DependencyIssue`, `DependencyHealth`, `ModRecord`, `SelectedModView`, `CatalogIssue`, and `StatusKind`.

- [ ] **Step 1: Write failing model tests**

Add `mod model;` to `src/lib.rs`, create `src/model.rs`, and begin with tests that require status priority and selected-view display overrides:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_priority_is_update_warning_restart_none() {
        assert!(StatusKind::Update.priority() > StatusKind::Warning.priority());
        assert!(StatusKind::Warning.priority() > StatusKind::RestartRequired.priority());
        assert!(StatusKind::RestartRequired.priority() > StatusKind::None.priority());
    }

    #[test]
    fn selected_view_uses_display_overrides_individually() {
        let record = ModRecord::test_record("demo", "Baseline", "Base Author").with_display(
            DisplayMetadata {
                title: Some("Override".to_owned()),
                author: None,
                version: Some("2.0.0".to_owned()),
                summary: None,
            },
        );
        let view = SelectedModView::from(&record);
        assert_eq!(view.name, "Override");
        assert_eq!(view.author, "Base Author");
        assert_eq!(view.version, "2.0.0");
        assert_eq!(view.description, record.identity.description);
    }
}
```

- [ ] **Step 2: Run the focused tests and confirm they fail**

Run:

```powershell
cargo test --locked model::tests -- --nocapture
```

Expected: compilation fails because the model types and methods do not exist.

- [ ] **Step 3: Implement the model types**

Define the types with `pub(crate)` visibility. Use this shape consistently in later tasks:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ModSource { Local, Workshop }

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ModDependency {
    pub mod_id: String,
    pub version: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ModIdentity {
    pub mod_id: String,
    pub name: String,
    pub author: String,
    pub version: String,
    pub description: String,
    pub dependencies: Vec<ModDependency>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct DisplayMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub version: Option<String>,
    pub summary: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StorageKind { Mod, GameData }

#[derive(Clone, Debug)]
pub(crate) enum IntegrationControl {
    Toggle { key: String, label: String, category: String, description: String, default: bool },
    Choice { key: String, label: String, category: String, description: String, options: Vec<ChoiceOption> },
    Button { action: String, label: String, category: String, description: String, button_label: String },
    FileCards { action: String, label: String, category: String, description: String, directory: String, filename_contains: String, extension: String, limit: usize },
}

#[derive(Clone, Debug)]
pub(crate) struct ModRecord {
    pub root: PathBuf,
    pub source: ModSource,
    pub identity: ModIdentity,
    pub display: DisplayMetadata,
    pub enabled: bool,
    pub assets: ModAssets,
    pub profile: Option<AuthorProfile>,
    pub integration: Option<IntegrationManifest>,
    pub dependency_summary: String,
    pub dependency_health: DependencyHealth,
}

#[derive(Clone, Debug)]
pub(crate) struct ChoiceOption { pub label: String, pub value: Value }

#[derive(Clone, Debug)]
pub(crate) struct IntegrationManifest {
    pub mod_id: String,
    pub name: String,
    pub storage: StorageKind,
    pub settings_file: String,
    pub actions_file: String,
    pub summary: String,
    pub display: DisplayMetadata,
    pub controls: Vec<IntegrationControl>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ModAssets {
    pub thumbnail: Option<String>,
    pub banner: Option<String>,
    pub profile_icon: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct AuthorProfile {
    pub display_name: String,
    pub bio: String,
    pub profile_icon: Option<String>,
    pub github_url: Option<String>,
    pub youtube_url: Option<String>,
    pub discord_contact: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DependencyIssueKind { Missing, Disabled, Incompatible, Malformed }

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DependencyIssue { pub kind: DependencyIssueKind, pub message: String }

#[derive(Clone, Debug, Default)]
pub(crate) struct DependencyHealth {
    pub warning: bool,
    pub tooltip: String,
    pub issues: Vec<DependencyIssue>,
}

#[derive(Clone, Debug)]
pub(crate) struct SelectedModView {
    pub name: String,
    pub author: String,
    pub version: String,
    pub description: String,
    pub source: ModSource,
    pub enabled: bool,
    pub dependency_summary: String,
    pub dependency_health: DependencyHealth,
    pub assets: ModAssets,
    pub profile: Option<AuthorProfile>,
    pub controls: Vec<IntegrationControl>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CatalogIssue {
    pub mod_id: Option<String>,
    pub file: PathBuf,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StatusKind { None, RestartRequired, Warning, Update }
```

Implement `StatusKind::priority() -> u8`, `ModRecord::search_text()`, `ModRecord::matches(query, source)`, and `From<&ModRecord> for SelectedModView`. Keep the `test_record` constructor behind `#[cfg(test)]`.

- [ ] **Step 4: Run model tests and formatting**

Run:

```powershell
cargo fmt --check
cargo test --locked model::tests -- --nocapture
```

Expected: both tests pass and formatting is clean.

- [ ] **Step 5: Commit the domain model**

```powershell
git add src/model.rs src/lib.rs
git commit -m "Add Better Mod Menu domain model"
```

### Task 2: Optional Integration and Profile Parsing

**Files:**
- Create: `src/integration.rs`
- Modify: `src/lib.rs`
- Test: `src/integration.rs` unit tests

**Interfaces:**
- Consumes: `ModIdentity`, `IntegrationManifest`, `AuthorProfile`, `ModAssets`, and `CatalogIssue` from `model`.
- Produces: `parse_manifest(root, expected_mod_id, source)`, `parse_profile(root, fallback_author, source)`, `load_optional_manifest(root, identity)`, `load_optional_profile(root, identity)`, `discover_assets(root, mod_id, profile)`, `canonical_github_url`, `canonical_youtube_url`, and `validated_discord_contact`.

- [ ] **Step 1: Write failing parser tests**

Create tests using explicit JSON strings and a process-unique temporary directory helper:

```rust
#[test]
fn valid_manifest_parses_all_control_kinds() {
    let root = fixture_dir("manifest-all-controls");
    let source = r#"{
      "schema_version":1,"mod_id":"demo","name":"Demo","storage":"game_data",
      "controls":[
        {"type":"toggle","key":"enabled","label":"Enabled","default":true},
        {"type":"choice","key":"mode","label":"Mode","options":[{"label":"Safe","value":"safe"}]},
        {"type":"button","action":"refresh","label":"Refresh"},
        {"type":"file_cards","action":"import","label":"Backups","directory":"backups","limit":5}
      ]
    }"#;
    let parsed = parse_manifest(&root, "demo", source).unwrap();
    assert_eq!(parsed.controls.len(), 4);
    assert_eq!(parsed.storage, StorageKind::GameData);
}

#[test]
fn invalid_optional_manifest_does_not_change_baseline_identity() {
    let root = fixture_dir("invalid-manifest");
    std::fs::write(root.join("better_mod_menu.json"), "{not json").unwrap();
    let identity = identity("demo", "Baseline");
    let (manifest, issues) = load_optional_manifest(&root, &identity);
    assert!(manifest.is_none());
    assert_eq!(identity.name, "Baseline");
    assert_eq!(issues.len(), 1);
}

#[test]
fn profile_accepts_only_trusted_provider_values() {
    let root = fixture_dir("profile-validation");
    std::fs::write(root.join("profile_icon.png"), b"png").unwrap();
    let valid = r#"{"schema_version":1,"display_name":"Author","profile_icon":"profile_icon.png","bio":"Short bio","links":{"github":"MadManPetr1","youtube":"@l95_madmanpetr1","discord":"l95madmanpetr1"}}"#;
    let profile = parse_profile(&root, "Fallback", valid).unwrap();
    assert_eq!(profile.github_url.as_deref(), Some("https://github.com/MadManPetr1"));
    assert_eq!(profile.youtube_url.as_deref(), Some("https://www.youtube.com/@l95_madmanpetr1"));
    assert_eq!(profile.discord_contact.as_deref(), Some("l95madmanpetr1"));
    assert!(parse_profile(&root, "Fallback", r#"{"schema_version":1,"links":{"github":"https://example.com"}}"#).is_err());
}
```

Add tests for schema version, mismatched mod id, unknown fields, duplicate keys, empty choice options, control-count bounds, 241-character bio, invalid icon name, missing icon file, and disabled-mod asset discovery.

- [ ] **Step 2: Run parser tests and confirm failure**

```powershell
cargo test --locked integration::tests -- --nocapture
```

Expected: compilation fails because `integration` and parser functions are absent.

- [ ] **Step 3: Implement strict serde input types and validation**

Add `mod integration;` to `src/lib.rs`. Use private `#[derive(Deserialize)]` types with `#[serde(deny_unknown_fields)]` and convert them into model types only after validation. Implement defaults exactly as the schemas specify:

```rust
fn default_storage() -> String { "mod".to_owned() }
fn default_settings_file() -> String { "settings.json".to_owned() }
fn default_actions_file() -> String { "better_mod_menu.actions.json".to_owned() }
fn default_button_label() -> String { "Run".to_owned() }
fn default_file_card_limit() -> usize { 5 }
```

Validate safe single-component mod ids, one-to-seven controls, non-empty keys/actions, unique setting keys, non-empty choice options, and limits from one through five. `load_optional_*` must return `(Option<T>, Vec<CatalogIssue>)`, where missing files return `(None, Vec::new())` and invalid files return one issue.

`load_optional_manifest` reads only `<mod root>/better_mod_menu.json` and
`load_optional_profile` reads only `<mod root>/better_mod_menu_profile.json`.

Provider URL constructors must build URLs from validated handles rather than accepting URLs. Asset discovery must check files independently of `enabled` and return source strings such as `asset/demo/thumbnail` only for existing candidates.

Use these exact public-within-crate signatures:

```rust
pub(crate) fn parse_manifest(root: &Path, expected_mod_id: &str, source: &str) -> Result<IntegrationManifest, String>;
pub(crate) fn parse_profile(root: &Path, fallback_author: &str, source: &str) -> Result<AuthorProfile, String>;
pub(crate) fn load_optional_manifest(root: &Path, identity: &ModIdentity) -> (Option<IntegrationManifest>, Vec<CatalogIssue>);
pub(crate) fn load_optional_profile(root: &Path, identity: &ModIdentity) -> (Option<AuthorProfile>, Vec<CatalogIssue>);
pub(crate) fn discover_assets(root: &Path, mod_id: &str, profile: Option<&AuthorProfile>) -> ModAssets;
pub(crate) fn canonical_github_url(value: &str) -> Option<String>;
pub(crate) fn canonical_youtube_url(value: &str) -> Option<String>;
pub(crate) fn validated_discord_contact(value: &str) -> Option<String>;
```

- [ ] **Step 4: Run parser tests and the existing suite**

```powershell
cargo fmt --check
cargo test --locked integration::tests -- --nocapture
cargo test --locked
```

Expected: parser tests and all existing tests pass.

- [ ] **Step 5: Commit optional integration parsing**

```powershell
git add src/integration.rs src/lib.rs
git commit -m "Parse optional Better Mod Menu integration files"
```

### Task 3: Dependency Requirements and Health

**Files:**
- Create: `src/dependencies.rs`
- Modify: `src/lib.rs`
- Test: `src/dependencies.rs` unit tests

**Interfaces:**
- Consumes: `ModDependency`, `DependencyHealth`, and `DependencyIssue` from `model`.
- Produces: `InstalledDependency`, `compare_versions`, `parse_requirement`, `friendly_requirement`, `friendly_dependency_text`, and `evaluate_dependencies(dependencies, installed, enabled, game_version)`.

- [ ] **Step 1: Write failing dependency tests**

```rust
#[test]
fn evaluates_supported_multi_clause_requirements() {
    assert!(version_satisfies("0.6.0", ">=0.6.0").unwrap());
    assert!(version_satisfies("0.5.3", ">=0.5.2, <0.5.4").unwrap());
    assert!(!version_satisfies("0.5.4", ">=0.5.2, <0.5.4").unwrap());
}

#[test]
fn malformed_clause_becomes_a_dependency_warning() {
    let dependencies = vec![ModDependency { mod_id: "base".to_owned(), version: ">>0.6".to_owned() }];
    let health = evaluate_dependencies(&dependencies, &HashMap::new(), &HashSet::new(), "0.6.0");
    assert!(health.warning);
    assert!(health.tooltip.contains(">>0.6"));
}

#[test]
fn reports_missing_disabled_and_incompatible_mods() {
    let installed = HashMap::from([
        ("disabled".to_owned(), InstalledDependency::new("Disabled Mod", "1.0.0")),
        ("old".to_owned(), InstalledDependency::new("Old Mod", "1.0.0")),
    ]);
    let dependencies = vec![
        dependency("missing", ">=1.0.0"),
        dependency("disabled", ">=1.0.0"),
        dependency("old", ">=2.0.0"),
    ];
    let health = evaluate_dependencies(&dependencies, &installed, &HashSet::new(), "0.6.0");
    assert_eq!(health.issues.len(), 3);
}
```

Also test equality, greater/less bounds, `v` prefixes, uneven version lengths, friendly base text, and a fully healthy dependency set.

- [ ] **Step 2: Run dependency tests and confirm failure**

```powershell
cargo test --locked dependencies::tests -- --nocapture
```

Expected: compilation fails because dependency helpers do not exist.

- [ ] **Step 3: Implement version and health evaluation**

Add `mod dependencies;` to `src/lib.rs`. Parse versions into numeric components after trimming a leading `v`; resize both vectors to equal length before comparison. Parse only `=`, `>`, `>=`, `<`, and `<=`. Return `Result<bool, RequirementError>` so malformed clauses cannot become healthy accidentally.

Use this stable lookup interface:

```rust
#[derive(Clone, Debug)]
pub(crate) struct InstalledDependency {
    pub display_name: String,
    pub version: String,
}

pub(crate) fn evaluate_dependencies(
    dependencies: &[ModDependency],
    installed: &HashMap<String, InstalledDependency>,
    enabled: &HashSet<String>,
    game_version: &str,
) -> DependencyHealth;
```

Base uses `game_version`; other ids use `installed`. Join issue messages with `"  |  "`. Healthy dependencies return `warning: false` and the exact tooltip `"Dependencies are installed, enabled, and compatible."`.

- [ ] **Step 4: Run dependency and full tests**

```powershell
cargo fmt --check
cargo test --locked dependencies::tests -- --nocapture
cargo test --locked
```

Expected: all dependency cases and the existing suite pass.

- [ ] **Step 5: Commit dependency evaluation**

```powershell
git add src/dependencies.rs src/lib.rs
git commit -m "Add readable dependency health evaluation"
```

### Task 4: Atomic JSON I/O and Enabled-Mod Mutation

**Files:**
- Create: `src/io.rs`
- Modify: `src/lib.rs`
- Test: `src/io.rs` unit tests

**Interfaces:**
- Consumes: `serde_json::{Map, Value}` and filesystem paths.
- Produces: `read_json_value`, `read_json_object`, `write_json_object_atomic`, `set_enabled_in_document`, and `update_enabled_mods_file`.

- [ ] **Step 1: Write failing atomic-write and configuration tests**

```rust
#[test]
fn atomic_write_replaces_existing_object() {
    let root = fixture_dir("atomic-replace");
    let path = root.join("settings.json");
    std::fs::write(&path, b"{\"old\":true}\n").unwrap();
    let values = Map::from_iter([("new".to_owned(), Value::Bool(true))]);
    write_json_object_atomic(&path, &values).unwrap();
    assert_eq!(read_json_object(&path).unwrap(), values);
    assert!(std::fs::read_dir(&root).unwrap().all(|entry| !entry.unwrap().file_name().to_string_lossy().contains(".tmp-")));
}

#[test]
fn non_object_json_is_not_overwritten() {
    let root = fixture_dir("non-object");
    let path = root.join("settings.json");
    std::fs::write(&path, b"[1,2,3]\n").unwrap();
    assert!(read_json_object(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"[1,2,3]\n");
}

#[test]
fn replacement_failure_preserves_existing_destination() {
    let root = fixture_dir("replace-failure");
    let path = root.join("settings.json");
    std::fs::write(&path, b"{\"old\":true}\n").unwrap();
    let values = Map::from_iter([("new".to_owned(), Value::Bool(true))]);
    let result = write_json_object_with_replace(&path, &values, |_temporary, _destination| {
        Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "simulated"))
    });
    assert!(result.is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"{\"old\":true}\n");
    assert!(std::fs::read_dir(&root).unwrap().all(|entry| !entry.unwrap().file_name().to_string_lossy().contains(".tmp-")));
}

#[test]
fn bulk_disable_preserves_bmm_and_unknown_fields() {
    let mut document = json!({"enabled_mods":["a","tfm2_better_mod_menu","b"],"other":42});
    set_enabled_in_document(&mut document, &["a".to_owned(), "b".to_owned()], false, Some("tfm2_better_mod_menu")).unwrap();
    assert_eq!(document, json!({"enabled_mods":["tfm2_better_mod_menu"],"other":42}));
}
```

Add a test-only replacement adapter that simulates replacement failure and asserts the pre-existing destination bytes remain unchanged.

- [ ] **Step 2: Run I/O tests and confirm failure**

```powershell
cargo test --locked io::tests -- --nocapture
```

Expected: compilation fails because the I/O module is absent.

- [ ] **Step 3: Implement the JSON boundary and Windows replacement**

Add `mod io;` to `src/lib.rs`. Serialize pretty JSON with one trailing newline. Create the destination parent, write a uniquely named sibling temporary file using `create_new(true)`, call `sync_all`, then replace the destination.

Keep replacement independently testable through this internal seam:

```rust
fn write_json_object_with_replace(
    path: &Path,
    values: &Map<String, Value>,
    replace: impl FnOnce(&Path, &Path) -> std::io::Result<()>,
) -> Result<(), String>;

pub(crate) fn write_json_object_atomic(
    path: &Path,
    values: &Map<String, Value>,
) -> Result<(), String>;
```

On Windows, use `MoveFileExW` with these constants and no shell command:

```rust
const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
const MOVEFILE_WRITE_THROUGH: u32 = 0x8;

#[link(name = "kernel32")]
extern "system" {
    fn MoveFileExW(existing: *const u16, replacement: *const u16, flags: u32) -> i32;
}
```

If replacement fails, capture `std::io::Error::last_os_error()`, attempt to remove only the known sibling temporary file, and return the error. For a missing destination, the same call works without `MOVEFILE_REPLACE_EXISTING`; for non-Windows test compilation, use `std::fs::rename` after ensuring the destination does not exist.

Move the current `set_enabled_in_document` behavior into this module and add `preserve_id: Option<&str>`. `update_enabled_mods_file` must reload the latest document immediately before mutation and write through `write_json_object_atomic`.

- [ ] **Step 4: Run I/O and full tests**

```powershell
cargo fmt --check
cargo test --locked io::tests -- --nocapture
cargo test --locked
```

Expected: replacement, failure preservation, and enabled-mod tests pass.

- [ ] **Step 5: Commit atomic JSON I/O**

```powershell
git add src/io.rs src/lib.rs
git commit -m "Add atomic JSON persistence for mod state"
```

### Task 5: Generic Settings, Actions, and File Cards

**Files:**
- Create: `src/settings.rs`
- Modify: `src/lib.rs`
- Test: `src/settings.rs` unit tests

**Interfaces:**
- Consumes: `ModRecord`, `IntegrationControl`, `StorageKind`, and atomic JSON functions.
- Produces: `SettingCommand`, `CommandOutcome`, `resolve_storage_root`, `resolve_safe_relative`, `load_settings`, `apply_setting_command`, and `discover_file_cards`.

- [ ] **Step 1: Write failing settings and path tests**

```rust
#[test]
fn safe_relative_paths_reject_escape_forms() {
    let root = fixture_dir("safe-paths");
    assert_eq!(resolve_safe_relative(&root, "nested/settings.json"), Some(root.join("nested/settings.json")));
    assert!(resolve_safe_relative(&root, "../outside.json").is_none());
    assert!(resolve_safe_relative(&root, "/rooted.json").is_none());
    assert!(resolve_safe_relative(&root, r"C:\outside.json").is_none());
    assert!(resolve_safe_relative(&root, r"nested\..\outside.json").is_none());
}

#[test]
fn toggle_write_preserves_unknown_values() {
    let fixture = settings_fixture();
    std::fs::write(&fixture.settings_path, b"{\"toggle\":false,\"owned_by_mod\":7}\n").unwrap();
    apply_setting_command(&fixture.record, &fixture.game_data_root, 0, SettingCommand::SetToggle(true)).unwrap();
    assert_eq!(read_json_object(&fixture.settings_path).unwrap(), Map::from_iter([
        ("toggle".to_owned(), Value::Bool(true)),
        ("owned_by_mod".to_owned(), Value::from(7)),
    ]));
}

#[test]
fn action_requests_use_the_public_payload() {
    let fixture = settings_fixture();
    apply_setting_command(&fixture.record, &fixture.game_data_root, 2, SettingCommand::Run).unwrap();
    assert_eq!(read_json_value(&fixture.actions_path).unwrap(), json!({"action":"refresh"}));
}

#[test]
fn setting_command_refuses_non_object_json_without_replacing_it() {
    let fixture = settings_fixture();
    std::fs::write(&fixture.settings_path, b"[1,2,3]\n").unwrap();
    assert!(apply_setting_command(&fixture.record, &fixture.game_data_root, 0, SettingCommand::SetToggle(true)).is_err());
    assert_eq!(std::fs::read(&fixture.settings_path).unwrap(), b"[1,2,3]\n");
}

#[test]
fn file_cards_are_filtered_newest_first_and_clamped_to_five() {
    let fixture = file_card_fixture_with_six_matching_files();
    let cards = discover_file_cards(&fixture.game_data_root, &fixture.control).unwrap();
    assert_eq!(cards.len(), 5);
    assert!(cards.windows(2).all(|pair| pair[0].modified >= pair[1].modified));
}
```

Add choice-index validation, command/control mismatch, default-without-write, button payload, file-card index payload, extension filtering, and non-object settings refusal tests.

- [ ] **Step 2: Run settings tests and confirm failure**

```powershell
cargo test --locked settings::tests -- --nocapture
```

Expected: compilation fails because settings interfaces do not exist.

- [ ] **Step 3: Implement settings commands and file cards**

Add `mod settings;` to `src/lib.rs` and define:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SettingCommand {
    SetToggle(bool),
    SelectChoice(usize),
    Run,
    RunFile(usize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CommandOutcome {
    pub message: String,
    pub refresh_file_cards: bool,
}
```

Resolve `StorageKind::Mod` to `record.root` and `StorageKind::GameData` to `<game_data_root>/<mod_id>`. Validate the command against the indexed control before reading or writing. Toggle and choice commands update only their key; button and file-card commands write the exact public action payload. A file-card command validates the selected index against a freshly discovered list.

`discover_file_cards` must reject unsafe directories, inspect regular files only, compare extensions case-insensitively without the leading dot, apply `filename_contains`, sort descending by modification time with filename as a stable tie-breaker, and clamp to `limit.min(5)`.

- [ ] **Step 4: Run settings and full tests**

```powershell
cargo fmt --check
cargo test --locked settings::tests -- --nocapture
cargo test --locked
```

Expected: all settings, action, path, and file-card cases pass.

- [ ] **Step 5: Commit the settings service**

```powershell
git add src/settings.rs src/lib.rs
git commit -m "Add generic manifest settings commands"
```

### Task 6: Deterministic Local and Workshop Catalog

**Files:**
- Create: `src/catalog.rs`
- Modify: `src/lib.rs`
- Test: `src/catalog.rs` unit tests

**Interfaces:**
- Consumes: model types, optional integration loaders, asset discovery, dependency evaluation, and enabled ids.
- Produces: `CatalogRoots`, `ModCatalog`, `CatalogBuild`, `runtime_roots(game_root)`, and `build_catalog(roots, enabled, game_version)`.

- [ ] **Step 1: Write failing catalog fixture tests**

```rust
#[test]
fn discovers_local_and_nested_workshop_mods() {
    let fixture = catalog_fixture();
    fixture.write_local_mod("local_mod", "Local Mod");
    fixture.write_workshop_mod("123456", "workshop_mod", "Workshop Mod");
    let build = build_catalog(&fixture.roots(), &HashSet::new(), "0.6.0");
    assert_eq!(build.catalog.records.len(), 2);
    assert_eq!(build.catalog.get("local_mod").unwrap().source, ModSource::Local);
    assert_eq!(build.catalog.get("workshop_mod").unwrap().source, ModSource::Workshop);
}

#[test]
fn local_duplicate_wins_and_reports_both_roots() {
    let fixture = catalog_fixture();
    fixture.write_local_mod("duplicate", "Local Copy");
    fixture.write_workshop_mod("999", "duplicate", "Workshop Copy");
    let build = build_catalog(&fixture.roots(), &HashSet::new(), "0.6.0");
    assert_eq!(build.catalog.get("duplicate").unwrap().identity.name, "Local Copy");
    assert!(build.issues.iter().any(|issue| issue.message.contains("duplicate") && issue.message.contains("Workshop")));
}

#[test]
fn corrupt_local_metadata_does_not_crash_catalog_build() {
    let fixture = catalog_fixture();
    fixture.write_raw_local("broken", "{bad json");
    fixture.write_workshop_mod("888", "healthy", "Healthy Workshop Mod");
    let build = build_catalog(&fixture.roots(), &HashSet::new(), "0.6.0");
    assert!(build.catalog.get("healthy").is_some());
    assert!(build.issues.iter().any(|issue| issue.file.ends_with("mod.mod_info")));
}

#[test]
fn corrupt_local_folder_does_not_hide_valid_workshop_mod() {
    let fixture = catalog_fixture();
    fixture.write_raw_local("duplicate", "{bad json");
    fixture.write_workshop_mod("777", "duplicate", "Workshop Copy");
    let build = build_catalog(&fixture.roots(), &HashSet::new(), "0.6.0");
    assert_eq!(build.catalog.get("duplicate").unwrap().identity.name, "Workshop Copy");
}
```

Add tests for missing roots, `base` exclusion, stable case-insensitive sorting with id tie-breaker, enabled state, optional-file enrichment, disabled asset discovery, display overrides, issue deduplication, and dependency health after all records exist.

- [ ] **Step 2: Run catalog tests and confirm failure**

```powershell
cargo test --locked catalog::tests -- --nocapture
```

Expected: compilation fails because catalog interfaces are absent.

- [ ] **Step 3: Implement two-pass catalog construction**

Add `mod catalog;` to `src/lib.rs` and define:

```rust
pub(crate) struct CatalogRoots {
    pub game_root: PathBuf,
    pub local_mods: PathBuf,
    pub workshop_mods: PathBuf,
    pub game_data: PathBuf,
}

pub(crate) struct ModCatalog {
    pub records: Vec<ModRecord>,
    by_id: HashMap<String, usize>,
}

pub(crate) struct CatalogBuild {
    pub catalog: ModCatalog,
    pub issues: Vec<CatalogIssue>,
}

pub(crate) fn runtime_roots(game_root: &Path) -> CatalogRoots;
pub(crate) fn build_catalog(
    roots: &CatalogRoots,
    enabled: &HashSet<String>,
    game_version: &str,
) -> CatalogBuild;
```

First pass reads required metadata, applies Local-before-Workshop deduplication, and enriches valid records with optional integration/profile/assets. Second pass builds `InstalledDependency` lookup values and assigns health to every record. Sort records only after enrichment and rebuild `by_id` after sorting.

`runtime_roots` derives Workshop from `<steamapps>/workshop/content/3009300` and game data from `%APPDATA%/TeamSamoyed/TeamfightManager2/data`. Missing `%APPDATA%` yields an issue and disables only `game_data` integration, not the catalog.

- [ ] **Step 4: Run catalog and full tests**

```powershell
cargo fmt --check
cargo test --locked catalog::tests -- --nocapture
cargo test --locked
```

Expected: catalog fixture tests and the complete suite pass.

- [ ] **Step 5: Commit catalog construction**

```powershell
git add src/catalog.rs src/lib.rs
git commit -m "Build deterministic Local and Workshop mod catalog"
```

### Task 7: Wire the Data Core into the Stable API Runtime

**Files:**
- Modify: `src/lib.rs`
- Modify: `scripts/validate_repo.ps1`
- Test: `src/lib.rs` unit tests and full project verification

**Interfaces:**
- Consumes: `build_catalog`, `runtime_roots`, `read_json_value`, `update_enabled_mods_file`, `ModRecord`, and `SelectedModView`.
- Produces: the existing `BetterModMenu` Stable API registration backed by `ModCatalog`, with unchanged current layout and event ids.

- [ ] **Step 1: Write failing runtime-adapter tests**

Replace the old `ModEntry` test helpers with `ModRecord::test_record` and add:

```rust
#[test]
fn active_search_and_source_filter_use_catalog_records() {
    let records = vec![
        ModRecord::test_record("local", "Intro Skip", "MadManPetr1"),
        ModRecord::test_record("workshop", "Workshop Match", "Author").with_source(ModSource::Workshop),
    ];
    assert_eq!(visible_indices_for(&records, "intro", SourceFilter::All), vec![0]);
    assert_eq!(visible_indices_for(&records, "match", SourceFilter::Workshop), vec![1]);
}

#[test]
fn selected_details_use_manifest_overrides_and_friendly_dependencies() {
    let record = detail_fixture_record();
    let view = SelectedModView::from(&record);
    assert_eq!(view.name, "Integrated Title");
    assert_eq!(view.dependency_summary, "TFM2 version >= 0.6.0");
    assert!(view.assets.thumbnail.is_some());
}
```

Run the tests before changing runtime storage.

- [ ] **Step 2: Confirm runtime-adapter tests fail**

```powershell
cargo test --locked tests::active_search_and_source_filter_use_catalog_records -- --nocapture
cargo test --locked tests::selected_details_use_manifest_overrides_and_friendly_dependencies -- --nocapture
```

Expected: failure because runtime helpers still consume the old `ModEntry` shape.

- [ ] **Step 3: Replace the old runtime data path**

In `src/lib.rs`:

- Remove the local `ModInfo`, `Dependency`, and `ModEntry` definitions.
- Change `SharedState.mods` to `Mutex<Vec<ModRecord>>`.
- Change filter helpers to accept `&[ModRecord]`.
- Build catalog during `init` using the executable's parent as game root and `host.game_version()` formatted as `major.minor.patch`.
- Log every unique `CatalogIssue` through `host.log(LogLevel::Warn, message)` after the build-identity log.
- Render list name/author/source from `SelectedModView` and `ModSource`.
- Render detail title, author, version, summary, dependency summary, thumbnail, and banner from the new record/view.
- Keep the fixed Intro Skip settings row functions and current UI paths unchanged.
- Replace direct `mods.json` parsing and writes with `io` module calls.
- Pass `Some(MOD_ID)` for bulk disable and `None` for individual changes; explicitly reject an individual request to disable Better Mod Menu only if the existing UI already prevents it, leaving self-disable policy for Phase 4 interaction review.
- Remove superseded scan, candidate, enabled-id, and JSON mutation helpers after their callers are migrated.

Use the Stable API game version instead of a hardcoded constant:

```rust
let game_version = host.game_version();
let game_version_text = format!("{}.{}.{}", game_version.major, game_version.minor, game_version.patch);
```

- [ ] **Step 4: Extend repository validation for the module boundary**

Add the six source modules to the required-file list in `scripts/validate_repo.ps1` and assert that `src/lib.rs` declares each module. Do not add UI or release-metadata requirements.

- [ ] **Step 5: Run complete static and unit verification**

```powershell
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
.\scripts\validate_repo.ps1
git diff --check
```

Expected: all tests pass, clippy emits no warnings, foundation validation passes, and the diff check is empty.

- [ ] **Step 6: Run Stable SDK build and staged installation verification**

```powershell
.\scripts\test_foundation.ps1 -SdkDir "D:\Game Clients\Steam\steamapps\common\Teamfight Manager2\mod-sdk-stable"
```

Expected: ABI 9 release build succeeds, staged DLL matches the canonical SHA-256, and the build fingerprint matches the current source tree.

- [ ] **Step 7: Commit runtime integration**

```powershell
git add src/lib.rs scripts/validate_repo.ps1
git commit -m "Use the generic data core in the Stable API menu"
```

- [ ] **Step 8: Rebuild from the clean commit and install the exact artifact locally**

```powershell
.\build_local.ps1 -SdkDir "D:\Game Clients\Steam\steamapps\common\Teamfight Manager2\mod-sdk-stable"
.\scripts\install_local.ps1 `
  -SdkDir "D:\Game Clients\Steam\steamapps\common\Teamfight Manager2\mod-sdk-stable" `
  -DestinationDir "D:\Game Clients\Steam\steamapps\common\Teamfight Manager2\mods\tfm2_better_mod_menu" `
  -SkipBuild
```

Expected: the live DLL and source DLL hashes match, the manifest reports `source_dirty: false`, and `config/game/mods.json` remains unchanged by installation.

- [ ] **Step 9: Perform the Phase 3 live smoke test**

Launch TFM2 0.6.0, enable Better Mod Menu through the native screen only if it is currently disabled, restart once, and verify:

1. Mods opens the current Better Mod Menu surface.
2. Local and Workshop labels remain correct.
3. Search and filters still select the expected rows.
4. A mod with neither optional JSON file still appears.
5. Intro Skip's existing fixed Settings view remains operational.
6. Thumbnail and banner remain visible for a disabled mod.
7. Close and Escape retain their current behavior.

Record observed results in the final task summary rather than adding a miscellaneous report file.

### Task 8: Whole-Phase Review and Handoff

**Files:**
- Review: all Phase 3 commits and `docs/superpowers/specs/2026-09-20-bmm-phase-3-data-core-design.md`
- Modify only if review finds a concrete defect.

**Interfaces:**
- Consumes: the completed Phase 3 branch.
- Produces: review findings resolved or explicitly reported, with no Phase 4 feature work mixed in.

- [ ] **Step 1: Review the branch against the spec**

Use a fresh reviewer to check:

- Every optional-file failure is non-fatal.
- Local precedence and Workshop nesting match the spec.
- Provider and path validation cannot accept arbitrary URLs or path escape.
- Settings and enabled-mod writes preserve unrelated values.
- Atomic replacement does not destroy an existing file on failure.
- Runtime wiring does not introduce visual or hitbox changes.

- [ ] **Step 2: Fix only validated Phase 3 defects**

For each concrete finding, add a failing regression test, run it to observe failure, implement the smallest fix, and rerun the focused test plus `cargo test --locked`. Do not add deferred Phase 4 functionality.

- [ ] **Step 3: Run final verification after review**

```powershell
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
.\scripts\validate_repo.ps1
.\scripts\test_foundation.ps1 -SdkDir "D:\Game Clients\Steam\steamapps\common\Teamfight Manager2\mod-sdk-stable"
git diff --check
git status --short --branch
```

Expected: every command passes and only ignored build outputs exist outside Git status.

- [ ] **Step 4: Commit review fixes when present**

If review produced tracked fixes:

```powershell
git add src scripts
git commit -m "Fix Phase 3 data core review findings"
```

If review produced no changes, do not create an empty commit.
