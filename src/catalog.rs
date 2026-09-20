// SPDX-License-Identifier: GPL-3.0-or-later
// See LICENSE-EXCEPTION.md for the TFM2 linking exception and attribution terms.

use crate::dependencies::{evaluate_dependencies, friendly_dependency_text, InstalledDependency};
use crate::integration::{discover_assets, load_optional_manifest, load_optional_profile};
use crate::model::{
    CatalogIssue, DependencyHealth, DisplayMetadata, ModDependency, ModIdentity, ModRecord,
    ModSource, SelectedModView,
};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub(crate) struct CatalogRoots {
    pub game_root: PathBuf,
    pub local_mods: PathBuf,
    pub workshop_mods: PathBuf,
    pub game_data: PathBuf,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ModCatalog {
    pub records: Vec<ModRecord>,
    by_id: HashMap<String, usize>,
}

impl ModCatalog {
    pub(crate) fn get(&self, mod_id: &str) -> Option<&ModRecord> {
        self.by_id
            .get(mod_id)
            .and_then(|index| self.records.get(*index))
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct CatalogBuild {
    pub catalog: ModCatalog,
    pub issues: Vec<CatalogIssue>,
}

#[derive(Deserialize)]
struct ModInfoInput {
    mod_id: String,
    name: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    dependencies: Vec<DependencyInput>,
}

#[derive(Deserialize)]
struct DependencyInput {
    mod_id: String,
    version: String,
}

pub(crate) fn runtime_roots(game_root: &Path) -> CatalogRoots {
    let steamapps = game_root
        .parent()
        .and_then(Path::parent)
        .unwrap_or(game_root);
    let game_data = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .map(|root| root.join("TeamSamoyed/TeamfightManager2/data"))
        .unwrap_or_default();
    CatalogRoots {
        game_root: game_root.to_path_buf(),
        local_mods: game_root.join("mods"),
        workshop_mods: steamapps.join("workshop/content/3009300"),
        game_data,
    }
}

pub(crate) fn build_catalog(
    roots: &CatalogRoots,
    enabled: &HashSet<String>,
    game_version: &str,
) -> CatalogBuild {
    let mut records: Vec<ModRecord> = Vec::new();
    let mut by_id = HashMap::<String, usize>::new();
    let mut issues = Vec::new();

    if roots.game_data.as_os_str().is_empty() {
        push_issue_unique(
            &mut issues,
            CatalogIssue {
                mod_id: None,
                file: roots.game_root.clone(),
                message: "APPDATA is unavailable; game-data integrations are disabled.".to_owned(),
            },
        );
    }

    for (source, candidates) in [
        (ModSource::Local, local_candidates(&roots.local_mods)),
        (
            ModSource::Workshop,
            workshop_candidates(&roots.workshop_mods),
        ),
    ] {
        for candidate in candidates {
            let metadata_path = candidate.join("mod.mod_info");
            let identity = match read_identity(&metadata_path) {
                Ok(identity) => identity,
                Err(message) => {
                    push_issue_unique(
                        &mut issues,
                        CatalogIssue {
                            mod_id: None,
                            file: metadata_path,
                            message,
                        },
                    );
                    continue;
                }
            };
            if identity.mod_id == "base" {
                continue;
            }
            if let Some(existing_index) = by_id.get(&identity.mod_id).copied() {
                let existing = &records[existing_index];
                let message = format!(
                    "duplicate mod id {}: Local {} takes precedence over Workshop {}",
                    identity.mod_id,
                    existing.root.display(),
                    candidate.display()
                );
                push_issue_unique(
                    &mut issues,
                    CatalogIssue {
                        mod_id: Some(identity.mod_id),
                        file: metadata_path,
                        message,
                    },
                );
                continue;
            }

            let (integration, manifest_issues) = load_optional_manifest(&candidate, &identity);
            for issue in manifest_issues {
                push_issue_unique(&mut issues, issue);
            }
            let (profile, profile_issues) = load_optional_profile(&candidate, &identity);
            for issue in profile_issues {
                push_issue_unique(&mut issues, issue);
            }
            let display = integration
                .as_ref()
                .map(|manifest| manifest.display.clone())
                .unwrap_or_else(DisplayMetadata::default);
            let assets = discover_assets(&candidate, &identity.mod_id, profile.as_ref());
            let mod_id = identity.mod_id.clone();
            let record = ModRecord {
                root: candidate,
                source,
                enabled: enabled.contains(&mod_id),
                identity,
                display,
                assets,
                profile,
                integration,
                dependency_summary: String::new(),
                dependency_health: DependencyHealth::default(),
            };
            by_id.insert(mod_id, records.len());
            records.push(record);
        }
    }

    let installed = records
        .iter()
        .map(|record| {
            let view = SelectedModView::from(record);
            (
                record.identity.mod_id.clone(),
                InstalledDependency::new(&view.name, &view.version),
            )
        })
        .collect::<HashMap<_, _>>();
    for record in &mut records {
        record.dependency_summary = record
            .identity
            .dependencies
            .iter()
            .map(|dependency| {
                let display_name = installed
                    .get(&dependency.mod_id)
                    .map(|dependency| dependency.display_name.as_str());
                friendly_dependency_text(dependency, display_name)
            })
            .collect::<Vec<_>>()
            .join("  |  ");
        record.dependency_health = evaluate_dependencies(
            &record.identity.dependencies,
            &installed,
            enabled,
            game_version,
        );
    }

    records.sort_by(|left, right| {
        let left_view = SelectedModView::from(left);
        let right_view = SelectedModView::from(right);
        left_view
            .name
            .to_lowercase()
            .cmp(&right_view.name.to_lowercase())
            .then_with(|| left.identity.mod_id.cmp(&right.identity.mod_id))
    });
    let by_id = records
        .iter()
        .enumerate()
        .map(|(index, record)| (record.identity.mod_id.clone(), index))
        .collect();
    CatalogBuild {
        catalog: ModCatalog { records, by_id },
        issues,
    }
}

fn read_identity(path: &Path) -> Result<ModIdentity, String> {
    let source = std::fs::read_to_string(path)
        .map_err(|error| format!("could not read required metadata: {error}"))?;
    let input: ModInfoInput = serde_json::from_str(&source)
        .map_err(|error| format!("invalid required metadata: {error}"))?;
    if input.mod_id.trim().is_empty() || input.name.trim().is_empty() {
        return Err("required metadata must contain non-empty mod_id and name".to_owned());
    }
    Ok(ModIdentity {
        mod_id: input.mod_id,
        name: input.name,
        author: input.author,
        version: input.version,
        description: input.description,
        dependencies: input
            .dependencies
            .into_iter()
            .map(|dependency| ModDependency {
                mod_id: dependency.mod_id,
                version: dependency.version,
            })
            .collect(),
    })
}

fn local_candidates(root: &Path) -> Vec<PathBuf> {
    let mut candidates = child_directories(root)
        .into_iter()
        .filter(|path| path.join("mod.mod_info").is_file())
        .collect::<Vec<_>>();
    candidates.sort();
    candidates
}

fn workshop_candidates(root: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for item in child_directories(root) {
        if item.join("mod.mod_info").is_file() {
            candidates.push(item.clone());
        }
        candidates.extend(
            child_directories(&item)
                .into_iter()
                .filter(|path| path.join("mod.mod_info").is_file()),
        );
    }
    candidates.sort();
    candidates.dedup();
    candidates
}

fn child_directories(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect()
}

fn push_issue_unique(issues: &mut Vec<CatalogIssue>, issue: CatalogIssue) {
    if !issues.iter().any(|existing| existing == &issue) {
        issues.push(issue);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DependencyIssueKind, ModSource, SelectedModView};
    use serde_json::json;
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

    struct CatalogFixture {
        root: PathBuf,
    }

    impl CatalogFixture {
        fn roots(&self) -> CatalogRoots {
            CatalogRoots {
                game_root: self.root.join("game"),
                local_mods: self.root.join("game/mods"),
                workshop_mods: self.root.join("workshop/content/3009300"),
                game_data: self.root.join("appdata/data"),
            }
        }

        fn write_mod(root: &Path, mod_id: &str, name: &str, dependencies: serde_json::Value) {
            std::fs::create_dir_all(root).unwrap();
            std::fs::write(
                root.join("mod.mod_info"),
                serde_json::to_vec_pretty(&json!({
                    "mod_id": mod_id,
                    "name": name,
                    "author": "Author",
                    "version": "1.0.0",
                    "description": "Description",
                    "dependencies": dependencies,
                }))
                .unwrap(),
            )
            .unwrap();
        }

        fn write_local_mod(&self, mod_id: &str, name: &str) -> PathBuf {
            let root = self.roots().local_mods.join(mod_id);
            Self::write_mod(&root, mod_id, name, json!([]));
            root
        }

        fn write_local_mod_with_dependencies(
            &self,
            mod_id: &str,
            name: &str,
            dependencies: serde_json::Value,
        ) -> PathBuf {
            let root = self.roots().local_mods.join(mod_id);
            Self::write_mod(&root, mod_id, name, dependencies);
            root
        }

        fn write_workshop_mod(&self, item: &str, mod_id: &str, name: &str) -> PathBuf {
            let root = self.roots().workshop_mods.join(item).join(mod_id);
            Self::write_mod(&root, mod_id, name, json!([]));
            root
        }

        fn write_raw_local(&self, folder: &str, source: &str) {
            let root = self.roots().local_mods.join(folder);
            std::fs::create_dir_all(&root).unwrap();
            std::fs::write(root.join("mod.mod_info"), source).unwrap();
        }
    }

    fn catalog_fixture() -> CatalogFixture {
        let id = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root =
            std::env::temp_dir().join(format!("tfm2-bmm-catalog-{}-{id}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        CatalogFixture { root }
    }

    #[test]
    fn discovers_local_and_nested_workshop_mods() {
        let fixture = catalog_fixture();
        fixture.write_local_mod("local_mod", "Local Mod");
        fixture.write_workshop_mod("123456", "workshop_mod", "Workshop Mod");
        let build = build_catalog(&fixture.roots(), &HashSet::new(), "0.6.0");
        assert_eq!(build.catalog.records.len(), 2);
        assert_eq!(
            build.catalog.get("local_mod").unwrap().source,
            ModSource::Local
        );
        assert_eq!(
            build.catalog.get("workshop_mod").unwrap().source,
            ModSource::Workshop
        );
    }

    #[test]
    fn local_duplicate_wins_and_reports_both_roots() {
        let fixture = catalog_fixture();
        fixture.write_local_mod("duplicate", "Local Copy");
        fixture.write_workshop_mod("999", "duplicate", "Workshop Copy");
        let build = build_catalog(&fixture.roots(), &HashSet::new(), "0.6.0");
        assert_eq!(
            build.catalog.get("duplicate").unwrap().identity.name,
            "Local Copy"
        );
        assert!(
            build
                .issues
                .iter()
                .any(|issue| issue.message.contains("duplicate")
                    && issue.message.contains("Workshop"))
        );
    }

    #[test]
    fn corrupt_local_metadata_does_not_crash_catalog_build() {
        let fixture = catalog_fixture();
        fixture.write_raw_local("broken", "{bad json");
        fixture.write_workshop_mod("888", "healthy", "Healthy Workshop Mod");
        let build = build_catalog(&fixture.roots(), &HashSet::new(), "0.6.0");
        assert!(build.catalog.get("healthy").is_some());
        assert!(build
            .issues
            .iter()
            .any(|issue| issue.file.ends_with("mod.mod_info")));
    }

    #[test]
    fn corrupt_local_folder_does_not_hide_valid_workshop_mod() {
        let fixture = catalog_fixture();
        fixture.write_raw_local("duplicate", "{bad json");
        fixture.write_workshop_mod("777", "duplicate", "Workshop Copy");
        let build = build_catalog(&fixture.roots(), &HashSet::new(), "0.6.0");
        assert_eq!(
            build.catalog.get("duplicate").unwrap().identity.name,
            "Workshop Copy"
        );
    }

    #[test]
    fn missing_roots_and_base_are_non_fatal() {
        let fixture = catalog_fixture();
        CatalogFixture::write_mod(
            &fixture.roots().local_mods.join("base"),
            "base",
            "Base",
            json!([]),
        );
        let build = build_catalog(&fixture.roots(), &HashSet::new(), "0.6.0");
        assert!(build.catalog.records.is_empty());
    }

    #[test]
    fn sorting_enabled_state_and_id_tie_break_are_stable() {
        let fixture = catalog_fixture();
        fixture.write_local_mod("z_id", "same");
        fixture.write_local_mod("a_id", "Same");
        let enabled = HashSet::from(["z_id".to_owned()]);
        let build = build_catalog(&fixture.roots(), &enabled, "0.6.0");
        assert_eq!(build.catalog.records[0].identity.mod_id, "a_id");
        assert_eq!(build.catalog.records[1].identity.mod_id, "z_id");
        assert!(build.catalog.get("z_id").unwrap().enabled);
        assert!(!build.catalog.get("a_id").unwrap().enabled);
    }

    #[test]
    fn optional_files_enrich_display_profile_and_disabled_assets() {
        let fixture = catalog_fixture();
        let root = fixture.write_local_mod("demo", "Baseline");
        std::fs::write(
            root.join("better_mod_menu.json"),
            r#"{"schema_version":1,"mod_id":"demo","name":"Demo","display":{"title":"Integrated Title","author":"Integrated Author"},"controls":[{"type":"toggle","key":"enabled","label":"Enabled"}]}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("better_mod_menu_profile.json"),
            r#"{"schema_version":1,"display_name":"Profile Author","bio":"Profile bio","links":{"github":"MadManPetr1"}}"#,
        )
        .unwrap();
        std::fs::write(root.join("thumbnail.png"), b"thumbnail").unwrap();
        std::fs::write(root.join("banner.png"), b"banner").unwrap();
        let build = build_catalog(&fixture.roots(), &HashSet::new(), "0.6.0");
        let record = build.catalog.get("demo").unwrap();
        let view = SelectedModView::from(record);
        assert_eq!(view.name, "Integrated Title");
        assert_eq!(view.author, "Integrated Author");
        assert!(record.profile.is_some());
        assert!(record.assets.thumbnail.is_some());
        assert!(record.assets.banner.is_some());
        assert!(!record.enabled);
    }

    #[test]
    fn dependency_health_is_evaluated_after_all_records_exist() {
        let fixture = catalog_fixture();
        fixture.write_local_mod_with_dependencies(
            "consumer",
            "Consumer",
            json!([{"mod_id":"provider","version":">=1.0.0"}]),
        );
        fixture.write_local_mod("provider", "Provider");
        let enabled = HashSet::from(["consumer".to_owned(), "provider".to_owned()]);
        let build = build_catalog(&fixture.roots(), &enabled, "0.6.0");
        let consumer = build.catalog.get("consumer").unwrap();
        assert!(!consumer.dependency_health.warning);
        assert_eq!(consumer.dependency_summary, "Requires Provider >= 1.0.0");
    }

    #[test]
    fn malformed_optional_file_is_one_non_fatal_issue() {
        let fixture = catalog_fixture();
        let root = fixture.write_local_mod("demo", "Demo");
        std::fs::write(root.join("better_mod_menu.json"), "{bad json").unwrap();
        let build = build_catalog(&fixture.roots(), &HashSet::new(), "0.6.0");
        assert!(build.catalog.get("demo").is_some());
        assert_eq!(build.issues.len(), 1);
        let unique = build
            .issues
            .iter()
            .map(|issue| (&issue.file, &issue.message))
            .collect::<HashSet<_>>();
        assert_eq!(unique.len(), build.issues.len());
        assert!(build.catalog.get("demo").unwrap().integration.is_none());
        assert!(build
            .catalog
            .get("demo")
            .unwrap()
            .dependency_health
            .issues
            .is_empty());
        assert!(!build.catalog.get("demo").unwrap().dependency_health.warning);
        assert!(!build
            .catalog
            .get("demo")
            .unwrap()
            .dependency_health
            .issues
            .iter()
            .any(|issue| issue.kind == DependencyIssueKind::Malformed));
    }
}
