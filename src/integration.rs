// SPDX-License-Identifier: GPL-3.0-or-later
// See LICENSE-EXCEPTION.md for the TFM2 linking exception and attribution terms.

use crate::model::{
    AuthorProfile, CatalogIssue, ChoiceOption, DisplayMetadata, IntegrationControl,
    IntegrationManifest, ModAssets, ModIdentity, StorageKind,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Component, Path};

const MANIFEST_FILE: &str = "better_mod_menu.json";
const PROFILE_FILE: &str = "better_mod_menu_profile.json";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestInput {
    #[serde(rename = "$schema")]
    schema: Option<String>,
    schema_version: u32,
    mod_id: String,
    name: String,
    #[serde(default = "default_storage")]
    storage: String,
    #[serde(default = "default_settings_file")]
    settings_file: String,
    #[serde(default = "default_actions_file")]
    actions_file: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    display: DisplayInput,
    controls: Vec<ControlInput>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct DisplayInput {
    title: Option<String>,
    author: Option<String>,
    version: Option<String>,
    summary: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum ControlInput {
    Toggle {
        key: String,
        label: String,
        #[serde(default)]
        category: String,
        #[serde(default)]
        description: String,
        #[serde(default)]
        default: bool,
    },
    Choice {
        key: String,
        label: String,
        #[serde(default)]
        category: String,
        #[serde(default)]
        description: String,
        options: Vec<ChoiceInput>,
    },
    Button {
        action: String,
        label: String,
        #[serde(default)]
        category: String,
        #[serde(default)]
        description: String,
        #[serde(default = "default_button_label")]
        button_label: String,
    },
    FileCards {
        action: String,
        label: String,
        #[serde(default)]
        category: String,
        #[serde(default)]
        description: String,
        directory: String,
        #[serde(default)]
        filename_contains: String,
        #[serde(default)]
        extension: String,
        #[serde(default = "default_file_card_limit")]
        limit: usize,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChoiceInput {
    label: String,
    value: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileInput {
    #[serde(rename = "$schema")]
    schema: Option<String>,
    schema_version: u32,
    display_name: Option<String>,
    profile_icon: Option<String>,
    #[serde(default)]
    bio: String,
    #[serde(default)]
    links: ProfileLinksInput,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileLinksInput {
    github: Option<String>,
    youtube: Option<String>,
    discord: Option<String>,
}

fn default_storage() -> String {
    "mod".to_owned()
}

fn default_settings_file() -> String {
    "settings.json".to_owned()
}

fn default_actions_file() -> String {
    "better_mod_menu.actions.json".to_owned()
}

fn default_button_label() -> String {
    "Run".to_owned()
}

fn default_file_card_limit() -> usize {
    5
}

fn non_empty(value: &str, field: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{field} must not be empty"))
    } else {
        Ok(())
    }
}

fn is_safe_mod_id(value: &str) -> bool {
    !value.is_empty()
        && !matches!(value, "." | "..")
        && !value.contains(['/', '\\'])
        && Path::new(value)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn is_safe_relative(value: &str) -> bool {
    if value.trim().is_empty() || value.contains('\\') {
        return false;
    }
    let path = Path::new(value);
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

pub(crate) fn parse_manifest(
    _root: &Path,
    expected_mod_id: &str,
    source: &str,
) -> Result<IntegrationManifest, String> {
    let input: ManifestInput =
        serde_json::from_str(source).map_err(|error| format!("invalid manifest JSON: {error}"))?;
    let _ = input.schema;

    if input.schema_version != 1 {
        return Err(format!(
            "unsupported manifest schema version {}",
            input.schema_version
        ));
    }
    if !is_safe_mod_id(&input.mod_id) || input.mod_id != expected_mod_id {
        return Err("manifest mod_id is unsafe or does not match mod.mod_info".to_owned());
    }
    non_empty(&input.name, "name")?;
    if !(1..=7).contains(&input.controls.len()) {
        return Err("controls must contain between one and seven entries".to_owned());
    }
    if !is_safe_relative(&input.settings_file) || !is_safe_relative(&input.actions_file) {
        return Err("settings_file and actions_file must be safe relative paths".to_owned());
    }

    let storage = match input.storage.as_str() {
        "mod" => StorageKind::Mod,
        "game_data" => StorageKind::GameData,
        _ => return Err(format!("unsupported storage value {}", input.storage)),
    };
    let mut setting_keys = HashSet::new();
    let mut controls = Vec::with_capacity(input.controls.len());
    for control in input.controls {
        let converted = match control {
            ControlInput::Toggle {
                key,
                label,
                category,
                description,
                default,
            } => {
                non_empty(&key, "toggle key")?;
                non_empty(&label, "toggle label")?;
                if !setting_keys.insert(key.clone()) {
                    return Err(format!("duplicate setting key {key}"));
                }
                IntegrationControl::Toggle {
                    key,
                    label,
                    category,
                    description,
                    default,
                }
            }
            ControlInput::Choice {
                key,
                label,
                category,
                description,
                options,
            } => {
                non_empty(&key, "choice key")?;
                non_empty(&label, "choice label")?;
                if !setting_keys.insert(key.clone()) {
                    return Err(format!("duplicate setting key {key}"));
                }
                if options.is_empty() {
                    return Err(format!("choice {key} must contain at least one option"));
                }
                let options = options
                    .into_iter()
                    .map(|option| {
                        non_empty(&option.label, "choice option label")?;
                        Ok(ChoiceOption {
                            label: option.label,
                            value: option.value,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                IntegrationControl::Choice {
                    key,
                    label,
                    category,
                    description,
                    options,
                }
            }
            ControlInput::Button {
                action,
                label,
                category,
                description,
                button_label,
            } => {
                non_empty(&action, "button action")?;
                non_empty(&label, "button label")?;
                non_empty(&button_label, "button button_label")?;
                IntegrationControl::Button {
                    action,
                    label,
                    category,
                    description,
                    button_label,
                }
            }
            ControlInput::FileCards {
                action,
                label,
                category,
                description,
                directory,
                filename_contains,
                extension,
                limit,
            } => {
                non_empty(&action, "file_cards action")?;
                non_empty(&label, "file_cards label")?;
                if !is_safe_relative(&directory) {
                    return Err("file_cards directory must be a safe relative path".to_owned());
                }
                if !(1..=5).contains(&limit) {
                    return Err("file_cards limit must be between one and five".to_owned());
                }
                IntegrationControl::FileCards {
                    action,
                    label,
                    category,
                    description,
                    directory,
                    filename_contains,
                    extension,
                    limit,
                }
            }
        };
        controls.push(converted);
    }

    Ok(IntegrationManifest {
        mod_id: input.mod_id,
        name: input.name,
        storage,
        settings_file: input.settings_file,
        actions_file: input.actions_file,
        summary: input.summary,
        display: DisplayMetadata {
            title: input.display.title,
            author: input.display.author,
            version: input.display.version,
            summary: input.display.summary,
        },
        controls,
    })
}

pub(crate) fn canonical_github_url(value: &str) -> Option<String> {
    let valid_length = (1..=39).contains(&value.len());
    let valid_chars = value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '-');
    let valid_edges = value
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_alphanumeric())
        && value
            .chars()
            .last()
            .is_some_and(|character| character.is_ascii_alphanumeric());
    (valid_length && valid_chars && valid_edges && !value.contains("--"))
        .then(|| format!("https://github.com/{value}"))
}

pub(crate) fn canonical_youtube_url(value: &str) -> Option<String> {
    let handle = value.strip_prefix('@').unwrap_or(value);
    let valid = (3..=30).contains(&handle.len())
        && handle.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '.' | '-')
        });
    valid.then(|| format!("https://www.youtube.com/@{handle}"))
}

pub(crate) fn validated_discord_contact(value: &str) -> Option<String> {
    let valid = (1..=32).contains(&value.len())
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '_' | '.')
        })
        && !value.starts_with('.')
        && !value.ends_with('.')
        && !value.contains("..");
    valid.then(|| value.to_owned())
}

pub(crate) fn parse_profile(
    root: &Path,
    fallback_author: &str,
    source: &str,
) -> Result<AuthorProfile, String> {
    let input: ProfileInput =
        serde_json::from_str(source).map_err(|error| format!("invalid profile JSON: {error}"))?;
    let _ = input.schema;
    if input.schema_version != 1 {
        return Err(format!(
            "unsupported profile schema version {}",
            input.schema_version
        ));
    }
    if input.bio.chars().count() > 240 {
        return Err("profile bio must not exceed 240 characters".to_owned());
    }
    let display_name = input
        .display_name
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| fallback_author.to_owned());
    if display_name.chars().count() > 80 {
        return Err("profile display_name must not exceed 80 characters".to_owned());
    }
    let profile_icon = match input.profile_icon {
        Some(icon) if matches!(icon.as_str(), "profile_icon.png" | "profile_icon.jpg") => {
            if !root.join(&icon).is_file() {
                return Err(format!("profile icon {icon} does not exist"));
            }
            Some(icon)
        }
        Some(_) => {
            return Err("profile_icon must be profile_icon.png or profile_icon.jpg".to_owned())
        }
        None => None,
    };
    let github_url = input
        .links
        .github
        .as_deref()
        .map(canonical_github_url)
        .transpose_option("invalid GitHub username")?;
    let youtube_url = input
        .links
        .youtube
        .as_deref()
        .map(canonical_youtube_url)
        .transpose_option("invalid YouTube handle")?;
    let discord_contact = input
        .links
        .discord
        .as_deref()
        .map(validated_discord_contact)
        .transpose_option("invalid Discord username")?;

    Ok(AuthorProfile {
        display_name,
        bio: input.bio,
        profile_icon,
        github_url,
        youtube_url,
        discord_contact,
    })
}

trait OptionalValidation<T> {
    fn transpose_option(self, message: &str) -> Result<Option<T>, String>;
}

impl<T> OptionalValidation<T> for Option<Option<T>> {
    fn transpose_option(self, message: &str) -> Result<Option<T>, String> {
        match self {
            Some(Some(value)) => Ok(Some(value)),
            Some(None) => Err(message.to_owned()),
            None => Ok(None),
        }
    }
}

pub(crate) fn load_optional_manifest(
    root: &Path,
    identity: &ModIdentity,
) -> (Option<IntegrationManifest>, Vec<CatalogIssue>) {
    load_optional(root, identity, MANIFEST_FILE, |source| {
        parse_manifest(root, &identity.mod_id, source)
    })
}

pub(crate) fn load_optional_profile(
    root: &Path,
    identity: &ModIdentity,
) -> (Option<AuthorProfile>, Vec<CatalogIssue>) {
    load_optional(root, identity, PROFILE_FILE, |source| {
        parse_profile(root, &identity.author, source)
    })
}

fn load_optional<T>(
    root: &Path,
    identity: &ModIdentity,
    file_name: &str,
    parse: impl FnOnce(&str) -> Result<T, String>,
) -> (Option<T>, Vec<CatalogIssue>) {
    let path = root.join(file_name);
    if !path.is_file() {
        return (None, Vec::new());
    }
    let result = std::fs::read_to_string(&path)
        .map_err(|error| format!("could not read {file_name}: {error}"))
        .and_then(|source| parse(&source));
    match result {
        Ok(value) => (Some(value), Vec::new()),
        Err(message) => (
            None,
            vec![CatalogIssue {
                mod_id: Some(identity.mod_id.clone()),
                file: path,
                message,
            }],
        ),
    }
}

pub(crate) fn discover_assets(
    root: &Path,
    mod_id: &str,
    profile: Option<&AuthorProfile>,
) -> ModAssets {
    fn first_asset(root: &Path, mod_id: &str, candidates: &[&str]) -> Option<String> {
        candidates.iter().find_map(|candidate| {
            root.join(candidate).is_file().then(|| {
                let source = candidate.strip_suffix(".png").unwrap_or(candidate);
                format!("asset/{mod_id}/{source}")
            })
        })
    }

    ModAssets {
        thumbnail: first_asset(root, mod_id, &["thumbnail.png", "assets/thumbnail.png"]),
        banner: first_asset(root, mod_id, &["banner.png", "assets/banner.png"]),
        profile_icon: profile.and_then(|profile| {
            profile.profile_icon.as_deref().and_then(|icon| {
                root.join(icon).is_file().then(|| {
                    let source = icon
                        .strip_suffix(".png")
                        .or_else(|| icon.strip_suffix(".jpg"))
                        .unwrap_or(icon);
                    format!("asset/{mod_id}/{source}")
                })
            })
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ModIdentity, StorageKind};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

    fn fixture_dir(name: &str) -> PathBuf {
        let id = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "tfm2-bmm-integration-{}-{name}-{id}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn identity(mod_id: &str, name: &str) -> ModIdentity {
        ModIdentity {
            mod_id: mod_id.to_owned(),
            name: name.to_owned(),
            author: "Fallback Author".to_owned(),
            version: "1.0.0".to_owned(),
            description: "Baseline".to_owned(),
            dependencies: Vec::new(),
        }
    }

    fn manifest_with_controls(controls: &str) -> String {
        format!(
            r#"{{"schema_version":1,"mod_id":"demo","name":"Demo","storage":"game_data","controls":[{controls}]}}"#
        )
    }

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
        assert_eq!(
            profile.github_url.as_deref(),
            Some("https://github.com/MadManPetr1")
        );
        assert_eq!(
            profile.youtube_url.as_deref(),
            Some("https://www.youtube.com/@l95_madmanpetr1")
        );
        assert_eq!(profile.discord_contact.as_deref(), Some("l95madmanpetr1"));
        assert!(parse_profile(
            &root,
            "Fallback",
            r#"{"schema_version":1,"links":{"github":"https://example.com"}}"#
        )
        .is_err());
    }

    #[test]
    fn manifest_rejects_bad_schema_id_unknown_fields_and_duplicate_setting_keys() {
        let root = fixture_dir("manifest-invalid");
        assert!(parse_manifest(
            &root,
            "demo",
            r#"{"schema_version":2,"mod_id":"demo","name":"Demo","controls":[{"type":"toggle","key":"a","label":"A"}]}"#
        )
        .is_err());
        assert!(parse_manifest(
            &root,
            "demo",
            r#"{"schema_version":1,"mod_id":"other","name":"Demo","controls":[{"type":"toggle","key":"a","label":"A"}]}"#
        )
        .is_err());
        assert!(parse_manifest(
            &root,
            "demo",
            r#"{"schema_version":1,"mod_id":"demo","name":"Demo","extra":true,"controls":[{"type":"toggle","key":"a","label":"A"}]}"#
        )
        .is_err());
        let duplicate = manifest_with_controls(
            r#"{"type":"toggle","key":"same","label":"A"},{"type":"choice","key":"same","label":"B","options":[{"label":"One","value":1}]}"#,
        );
        assert!(parse_manifest(&root, "demo", &duplicate).is_err());
    }

    #[test]
    fn manifest_requires_one_to_seven_valid_controls() {
        let root = fixture_dir("control-bounds");
        let empty = manifest_with_controls("");
        assert!(parse_manifest(&root, "demo", &empty).is_err());
        let empty_choice =
            manifest_with_controls(r#"{"type":"choice","key":"mode","label":"Mode","options":[]}"#);
        assert!(parse_manifest(&root, "demo", &empty_choice).is_err());
        let controls = (0..8)
            .map(|index| {
                format!(r#"{{"type":"toggle","key":"key{index}","label":"Toggle {index}"}}"#)
            })
            .collect::<Vec<_>>()
            .join(",");
        let eight = manifest_with_controls(&controls);
        assert!(parse_manifest(&root, "demo", &eight).is_err());
    }

    #[test]
    fn profile_enforces_bio_and_local_icon_contract() {
        let root = fixture_dir("profile-bounds");
        let long_bio = "a".repeat(241);
        let source = format!(r#"{{"schema_version":1,"bio":"{long_bio}"}}"#);
        assert!(parse_profile(&root, "Fallback", &source).is_err());
        assert!(parse_profile(
            &root,
            "Fallback",
            r#"{"schema_version":1,"profile_icon":"avatar.png"}"#
        )
        .is_err());
        assert!(parse_profile(
            &root,
            "Fallback",
            r#"{"schema_version":1,"profile_icon":"profile_icon.jpg"}"#
        )
        .is_err());
    }

    #[test]
    fn assets_are_discovered_without_enabled_state() {
        let root = fixture_dir("assets");
        std::fs::create_dir_all(root.join("assets")).unwrap();
        std::fs::write(root.join("assets/thumbnail.png"), b"thumbnail").unwrap();
        std::fs::write(root.join("banner.png"), b"banner").unwrap();
        let assets = discover_assets(&root, "demo", None);
        assert_eq!(
            assets.thumbnail.as_deref(),
            Some("asset/demo/assets/thumbnail")
        );
        assert_eq!(assets.banner.as_deref(), Some("asset/demo/banner"));
    }
}
