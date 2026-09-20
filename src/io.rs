// SPDX-License-Identifier: GPL-3.0-or-later
// See LICENSE-EXCEPTION.md for the TFM2 linking exception and attribution terms.

use serde_json::{Map, Value};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_TEMPORARY: AtomicUsize = AtomicUsize::new(0);

pub(crate) fn read_json_value(path: &Path) -> Result<Value, String> {
    let source = std::fs::read_to_string(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    serde_json::from_str(&source)
        .map_err(|error| format!("invalid JSON in {}: {error}", path.display()))
}

pub(crate) fn read_json_object(path: &Path) -> Result<Map<String, Value>, String> {
    match read_json_value(path)? {
        Value::Object(values) => Ok(values),
        _ => Err(format!("{} must contain a JSON object", path.display())),
    }
}

fn temporary_path(path: &Path, attempt: usize) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("bmm-json");
    path.with_file_name(format!(".{file_name}.tmp-{}-{attempt}", std::process::id()))
}

fn write_json_object_with_replace(
    path: &Path,
    values: &Map<String, Value>,
    replace: impl FnOnce(&Path, &Path) -> std::io::Result<()>,
) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("could not create {}: {error}", parent.display()))?;

    let mut serialized = serde_json::to_vec_pretty(values)
        .map_err(|error| format!("could not serialize {}: {error}", path.display()))?;
    serialized.push(b'\n');

    let temporary = loop {
        let attempt = NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed);
        let candidate = temporary_path(path, attempt);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                let write_result = file.write_all(&serialized).and_then(|_| file.sync_all());
                if let Err(error) = write_result {
                    drop(file);
                    let _ = std::fs::remove_file(&candidate);
                    return Err(format!(
                        "could not write temporary file {}: {error}",
                        candidate.display()
                    ));
                }
                break candidate;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "could not create temporary file {}: {error}",
                    candidate.display()
                ));
            }
        }
    };

    if let Err(error) = replace(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(format!(
            "could not replace {} with {}: {error}",
            path.display(),
            temporary.display()
        ));
    }
    Ok(())
}

pub(crate) fn write_json_object_atomic(
    path: &Path,
    values: &Map<String, Value>,
) -> Result<(), String> {
    write_json_object_with_replace(path, values, replace_file)
}

#[cfg(windows)]
fn replace_file(temporary: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;

    #[link(name = "kernel32")]
    extern "system" {
        fn MoveFileExW(existing: *const u16, replacement: *const u16, flags: u32) -> i32;
    }

    let existing = temporary
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let replacement = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let result = unsafe {
        MoveFileExW(
            existing.as_ptr(),
            replacement.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_file(temporary: &Path, destination: &Path) -> std::io::Result<()> {
    std::fs::rename(temporary, destination)
}

pub(crate) fn set_enabled_in_document(
    document: &mut Value,
    ids: &[String],
    enabled: bool,
    preserve_id: Option<&str>,
) -> Result<bool, String> {
    let entries = document
        .get_mut("enabled_mods")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "enabled_mods is missing or is not an array".to_owned())?;
    if entries.iter().any(|entry| !entry.is_string()) {
        return Err("enabled_mods must contain only strings".to_owned());
    }

    let previous = entries.clone();
    if enabled {
        for id in ids {
            if !entries.iter().any(|entry| entry.as_str() == Some(id)) {
                entries.push(Value::String(id.clone()));
            }
        }
    } else {
        entries.retain(|entry| {
            let Some(id) = entry.as_str() else {
                return true;
            };
            preserve_id == Some(id) || !ids.iter().any(|candidate| candidate == id)
        });
    }
    Ok(*entries != previous)
}

pub(crate) fn update_enabled_mods_file(
    path: &Path,
    ids: &[String],
    enabled: bool,
    preserve_id: Option<&str>,
) -> Result<bool, String> {
    let mut document = read_json_value(path)?;
    let changed = set_enabled_in_document(&mut document, ids, enabled, preserve_id)?;
    if !changed {
        return Ok(false);
    }
    let values = document
        .as_object()
        .ok_or_else(|| format!("{} must contain a JSON object", path.display()))?;
    write_json_object_atomic(path, values)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Map, Value};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

    fn fixture_dir(name: &str) -> PathBuf {
        let id = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root =
            std::env::temp_dir().join(format!("tfm2-bmm-io-{}-{name}-{id}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn atomic_write_replaces_existing_object() {
        let root = fixture_dir("atomic-replace");
        let path = root.join("settings.json");
        std::fs::write(&path, b"{\"old\":true}\n").unwrap();
        let values = Map::from_iter([("new".to_owned(), Value::Bool(true))]);
        write_json_object_atomic(&path, &values).unwrap();
        assert_eq!(read_json_object(&path).unwrap(), values);
        assert!(std::fs::read_dir(&root).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains(".tmp-")));
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
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "simulated",
            ))
        });
        assert!(result.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"{\"old\":true}\n");
        assert!(std::fs::read_dir(&root).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains(".tmp-")));
    }

    #[test]
    fn bulk_disable_preserves_bmm_and_unknown_fields() {
        let mut document = json!({"enabled_mods":["a","tfm2_better_mod_menu","b"],"other":42});
        set_enabled_in_document(
            &mut document,
            &["a".to_owned(), "b".to_owned()],
            false,
            Some("tfm2_better_mod_menu"),
        )
        .unwrap();
        assert_eq!(
            document,
            json!({"enabled_mods":["tfm2_better_mod_menu"],"other":42})
        );
    }
}
