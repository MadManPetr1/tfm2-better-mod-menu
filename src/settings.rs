// SPDX-License-Identifier: GPL-3.0-or-later
// See LICENSE-EXCEPTION.md for the TFM2 linking exception and attribution terms.

use crate::io::{read_json_object, write_json_object_atomic};
use crate::model::{IntegrationControl, ModRecord, StorageKind};
use serde_json::{Map, Value};
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

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

#[derive(Clone, Debug)]
pub(crate) struct FileCard {
    pub path: PathBuf,
    pub name: String,
    pub modified: SystemTime,
}

pub(crate) fn resolve_safe_relative(root: &Path, declared: &str) -> Option<PathBuf> {
    if declared.trim().is_empty() || declared.contains('\\') {
        return None;
    }
    let relative = Path::new(declared);
    if relative.is_absolute()
        || !relative
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return None;
    }
    Some(root.join(relative))
}

pub(crate) fn resolve_storage_root(
    record: &ModRecord,
    game_data_root: &Path,
) -> Result<PathBuf, String> {
    let manifest = record.integration.as_ref().ok_or_else(|| {
        format!(
            "{} has no Better Mod Menu integration",
            record.identity.mod_id
        )
    })?;
    Ok(match manifest.storage {
        StorageKind::Mod => record.root.clone(),
        StorageKind::GameData => game_data_root.join(&record.identity.mod_id),
    })
}

fn settings_path(record: &ModRecord, game_data_root: &Path) -> Result<PathBuf, String> {
    let manifest = record.integration.as_ref().ok_or_else(|| {
        format!(
            "{} has no Better Mod Menu integration",
            record.identity.mod_id
        )
    })?;
    let root = resolve_storage_root(record, game_data_root)?;
    resolve_safe_relative(&root, &manifest.settings_file)
        .ok_or_else(|| "manifest settings_file is not a safe relative path".to_owned())
}

fn actions_path(record: &ModRecord, game_data_root: &Path) -> Result<PathBuf, String> {
    let manifest = record.integration.as_ref().ok_or_else(|| {
        format!(
            "{} has no Better Mod Menu integration",
            record.identity.mod_id
        )
    })?;
    let root = resolve_storage_root(record, game_data_root)?;
    resolve_safe_relative(&root, &manifest.actions_file)
        .ok_or_else(|| "manifest actions_file is not a safe relative path".to_owned())
}

pub(crate) fn load_settings(
    record: &ModRecord,
    game_data_root: &Path,
) -> Result<Map<String, Value>, String> {
    let path = settings_path(record, game_data_root)?;
    if !path.exists() {
        return Ok(Map::new());
    }
    read_json_object(&path)
}

pub(crate) fn apply_setting_command(
    record: &ModRecord,
    game_data_root: &Path,
    control_index: usize,
    command: SettingCommand,
) -> Result<CommandOutcome, String> {
    let manifest = record.integration.as_ref().ok_or_else(|| {
        format!(
            "{} has no Better Mod Menu integration",
            record.identity.mod_id
        )
    })?;
    let control = manifest
        .controls
        .get(control_index)
        .ok_or_else(|| format!("control index {control_index} is out of range"))?;

    match (control, command) {
        (IntegrationControl::Toggle { key, .. }, SettingCommand::SetToggle(value)) => {
            let path = settings_path(record, game_data_root)?;
            let mut values = load_settings(record, game_data_root)?;
            values.insert(key.clone(), Value::Bool(value));
            write_json_object_atomic(&path, &values)?;
            Ok(CommandOutcome {
                message: "Setting saved.".to_owned(),
                refresh_file_cards: false,
            })
        }
        (
            IntegrationControl::Choice { key, options, .. },
            SettingCommand::SelectChoice(option_index),
        ) => {
            let option = options
                .get(option_index)
                .ok_or_else(|| format!("choice option index {option_index} is out of range"))?;
            let path = settings_path(record, game_data_root)?;
            let mut values = load_settings(record, game_data_root)?;
            values.insert(key.clone(), option.value.clone());
            write_json_object_atomic(&path, &values)?;
            Ok(CommandOutcome {
                message: "Setting saved.".to_owned(),
                refresh_file_cards: false,
            })
        }
        (IntegrationControl::Button { action, .. }, SettingCommand::Run) => {
            write_action(record, game_data_root, action, None)?;
            Ok(CommandOutcome {
                message: "Action requested.".to_owned(),
                refresh_file_cards: false,
            })
        }
        (IntegrationControl::FileCards { action, .. }, SettingCommand::RunFile(index)) => {
            let cards_root = game_data_root.join(&record.identity.mod_id);
            let cards = discover_file_cards(&cards_root, control)?;
            if index >= cards.len() {
                return Err(format!("file-card index {index} is out of range"));
            }
            write_action(record, game_data_root, action, Some(index))?;
            Ok(CommandOutcome {
                message: "File action requested.".to_owned(),
                refresh_file_cards: true,
            })
        }
        _ => Err("command does not match the selected control".to_owned()),
    }
}

fn write_action(
    record: &ModRecord,
    game_data_root: &Path,
    action: &str,
    index: Option<usize>,
) -> Result<(), String> {
    let path = actions_path(record, game_data_root)?;
    let mut values = Map::new();
    values.insert("action".to_owned(), Value::String(action.to_owned()));
    if let Some(index) = index {
        values.insert("index".to_owned(), Value::from(index));
    }
    write_json_object_atomic(&path, &values)
}

pub(crate) fn discover_file_cards(
    game_data_mod_root: &Path,
    control: &IntegrationControl,
) -> Result<Vec<FileCard>, String> {
    let IntegrationControl::FileCards {
        directory,
        filename_contains,
        extension,
        limit,
        ..
    } = control
    else {
        return Err("control is not a file_cards control".to_owned());
    };
    let directory = resolve_safe_relative(game_data_mod_root, directory)
        .ok_or_else(|| "file_cards directory is not a safe relative path".to_owned())?;
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let expected_extension = extension.trim_start_matches('.');
    let mut cards = Vec::new();
    let entries = std::fs::read_dir(&directory)
        .map_err(|error| format!("could not read {}: {error}", directory.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("could not read directory entry: {error}"))?;
        let metadata = entry.metadata().map_err(|error| {
            format!(
                "could not read {} metadata: {error}",
                entry.path().display()
            )
        })?;
        if !metadata.is_file() {
            continue;
        }
        let path = entry.path();
        let Some(name) = path
            .file_name()
            .and_then(|value| value.to_str())
            .map(str::to_owned)
        else {
            continue;
        };
        if !filename_contains.is_empty() && !name.contains(filename_contains) {
            continue;
        }
        let actual_extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if !expected_extension.is_empty()
            && !actual_extension.eq_ignore_ascii_case(expected_extension)
        {
            continue;
        }
        cards.push(FileCard {
            path,
            name,
            modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
        });
    }
    cards.sort_by(|left, right| {
        right
            .modified
            .cmp(&left.modified)
            .then_with(|| left.name.cmp(&right.name))
    });
    cards.truncate((*limit).min(5));
    Ok(cards)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::{read_json_object, read_json_value};
    use crate::model::{
        ChoiceOption, DisplayMetadata, IntegrationControl, IntegrationManifest, ModRecord,
        StorageKind,
    };
    use serde_json::{json, Map, Value};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

    fn fixture_dir(name: &str) -> PathBuf {
        let id = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "tfm2-bmm-settings-{}-{name}-{id}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    struct SettingsFixture {
        record: ModRecord,
        game_data_root: PathBuf,
        settings_path: PathBuf,
        actions_path: PathBuf,
    }

    fn controls() -> Vec<IntegrationControl> {
        vec![
            IntegrationControl::Toggle {
                key: "toggle".to_owned(),
                label: "Toggle".to_owned(),
                category: String::new(),
                description: String::new(),
                default: true,
            },
            IntegrationControl::Choice {
                key: "mode".to_owned(),
                label: "Mode".to_owned(),
                category: String::new(),
                description: String::new(),
                options: vec![
                    ChoiceOption {
                        label: "Safe".to_owned(),
                        value: Value::String("safe".to_owned()),
                    },
                    ChoiceOption {
                        label: "Fast".to_owned(),
                        value: Value::String("fast".to_owned()),
                    },
                ],
            },
            IntegrationControl::Button {
                action: "refresh".to_owned(),
                label: "Refresh".to_owned(),
                category: String::new(),
                description: String::new(),
                button_label: "Run".to_owned(),
            },
            IntegrationControl::FileCards {
                action: "import".to_owned(),
                label: "Backups".to_owned(),
                category: String::new(),
                description: String::new(),
                directory: "backups".to_owned(),
                filename_contains: "backup".to_owned(),
                extension: "sav".to_owned(),
                limit: 5,
            },
        ]
    }

    fn settings_fixture() -> SettingsFixture {
        let root = fixture_dir("commands");
        let game_data_root = root.join("game-data");
        let mut record = ModRecord::test_record("demo", "Demo", "Author");
        record.root = root.join("mod");
        record.integration = Some(IntegrationManifest {
            mod_id: "demo".to_owned(),
            name: "Demo".to_owned(),
            storage: StorageKind::GameData,
            settings_file: "settings.json".to_owned(),
            actions_file: "better_mod_menu.actions.json".to_owned(),
            summary: String::new(),
            display: DisplayMetadata::default(),
            controls: controls(),
        });
        let storage = game_data_root.join("demo");
        std::fs::create_dir_all(&storage).unwrap();
        SettingsFixture {
            record,
            game_data_root,
            settings_path: storage.join("settings.json"),
            actions_path: storage.join("better_mod_menu.actions.json"),
        }
    }

    #[test]
    fn safe_relative_paths_reject_escape_forms() {
        let root = fixture_dir("safe-paths");
        assert_eq!(
            resolve_safe_relative(&root, "nested/settings.json"),
            Some(root.join("nested/settings.json"))
        );
        assert!(resolve_safe_relative(&root, "../outside.json").is_none());
        assert!(resolve_safe_relative(&root, "/rooted.json").is_none());
        assert!(resolve_safe_relative(&root, r"C:\outside.json").is_none());
        assert!(resolve_safe_relative(&root, r"nested\..\outside.json").is_none());
    }

    #[test]
    fn toggle_write_preserves_unknown_values() {
        let fixture = settings_fixture();
        std::fs::write(
            &fixture.settings_path,
            b"{\"toggle\":false,\"owned_by_mod\":7}\n",
        )
        .unwrap();
        apply_setting_command(
            &fixture.record,
            &fixture.game_data_root,
            0,
            SettingCommand::SetToggle(true),
        )
        .unwrap();
        assert_eq!(
            read_json_object(&fixture.settings_path).unwrap(),
            Map::from_iter([
                ("toggle".to_owned(), Value::Bool(true)),
                ("owned_by_mod".to_owned(), Value::from(7)),
            ])
        );
    }

    #[test]
    fn action_requests_use_the_public_payload() {
        let fixture = settings_fixture();
        apply_setting_command(
            &fixture.record,
            &fixture.game_data_root,
            2,
            SettingCommand::Run,
        )
        .unwrap();
        assert_eq!(
            read_json_value(&fixture.actions_path).unwrap(),
            json!({"action":"refresh"})
        );
    }

    #[test]
    fn setting_command_refuses_non_object_json_without_replacing_it() {
        let fixture = settings_fixture();
        std::fs::write(&fixture.settings_path, b"[1,2,3]\n").unwrap();
        assert!(apply_setting_command(
            &fixture.record,
            &fixture.game_data_root,
            0,
            SettingCommand::SetToggle(true)
        )
        .is_err());
        assert_eq!(std::fs::read(&fixture.settings_path).unwrap(), b"[1,2,3]\n");
    }

    #[test]
    fn file_cards_are_filtered_newest_first_and_clamped_to_five() {
        let fixture = settings_fixture();
        let directory = fixture.game_data_root.join("demo/backups");
        std::fs::create_dir_all(&directory).unwrap();
        for index in 0..6 {
            std::fs::write(directory.join(format!("backup-{index}.sav")), [index]).unwrap();
        }
        std::fs::write(directory.join("backup-ignore.txt"), b"ignored").unwrap();
        std::fs::write(directory.join("other.sav"), b"ignored").unwrap();
        let control = &fixture.record.integration.as_ref().unwrap().controls[3];
        let cards = discover_file_cards(&fixture.game_data_root.join("demo"), control).unwrap();
        assert_eq!(cards.len(), 5);
        assert!(cards
            .windows(2)
            .all(|pair| pair[0].modified >= pair[1].modified));
        assert!(cards.iter().all(|card| card.name.contains("backup")
            && card.path.extension().and_then(|value| value.to_str()) == Some("sav")));
    }

    #[test]
    fn choice_and_command_validation_prevent_wrong_writes() {
        let fixture = settings_fixture();
        assert!(apply_setting_command(
            &fixture.record,
            &fixture.game_data_root,
            1,
            SettingCommand::SelectChoice(9)
        )
        .is_err());
        assert!(apply_setting_command(
            &fixture.record,
            &fixture.game_data_root,
            2,
            SettingCommand::SetToggle(true)
        )
        .is_err());
        assert!(!fixture.settings_path.exists());
    }

    #[test]
    fn missing_settings_use_virtual_defaults_without_writing() {
        let fixture = settings_fixture();
        assert!(load_settings(&fixture.record, &fixture.game_data_root)
            .unwrap()
            .is_empty());
        assert!(!fixture.settings_path.exists());
    }

    #[test]
    fn choice_and_file_card_commands_write_exact_values() {
        let fixture = settings_fixture();
        apply_setting_command(
            &fixture.record,
            &fixture.game_data_root,
            1,
            SettingCommand::SelectChoice(1),
        )
        .unwrap();
        assert_eq!(
            read_json_object(&fixture.settings_path).unwrap()["mode"],
            Value::String("fast".to_owned())
        );

        let directory = fixture.game_data_root.join("demo/backups");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("backup-1.sav"), b"save").unwrap();
        apply_setting_command(
            &fixture.record,
            &fixture.game_data_root,
            3,
            SettingCommand::RunFile(0),
        )
        .unwrap();
        assert_eq!(
            read_json_value(&fixture.actions_path).unwrap(),
            json!({"action":"import","index":0})
        );
    }
}
