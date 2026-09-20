// SPDX-License-Identifier: GPL-3.0-or-later
// See LICENSE-EXCEPTION.md for the TFM2 linking exception and attribution terms.

use serde_json::Value;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ModSource {
    Local,
    Workshop,
}

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
pub(crate) enum StorageKind {
    Mod,
    GameData,
}

#[derive(Clone, Debug)]
pub(crate) struct ChoiceOption {
    pub label: String,
    pub value: Value,
}

#[derive(Clone, Debug)]
pub(crate) enum IntegrationControl {
    Toggle {
        key: String,
        label: String,
        category: String,
        description: String,
        default: bool,
    },
    Choice {
        key: String,
        label: String,
        category: String,
        description: String,
        options: Vec<ChoiceOption>,
    },
    Button {
        action: String,
        label: String,
        category: String,
        description: String,
        button_label: String,
    },
    FileCards {
        action: String,
        label: String,
        category: String,
        description: String,
        directory: String,
        filename_contains: String,
        extension: String,
        limit: usize,
    },
}

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
pub(crate) enum DependencyIssueKind {
    Missing,
    Disabled,
    Incompatible,
    Malformed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DependencyIssue {
    pub kind: DependencyIssueKind,
    pub message: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct DependencyHealth {
    pub warning: bool,
    pub tooltip: String,
    pub issues: Vec<DependencyIssue>,
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
pub(crate) enum StatusKind {
    None,
    RestartRequired,
    Warning,
    Update,
}

impl StatusKind {
    pub(crate) const fn priority(self) -> u8 {
        match self {
            Self::None => 0,
            Self::RestartRequired => 1,
            Self::Warning => 2,
            Self::Update => 3,
        }
    }
}

impl ModRecord {
    pub(crate) fn search_text(&self) -> String {
        let view = SelectedModView::from(self);
        format!("{} {}", view.name, view.author).to_lowercase()
    }

    pub(crate) fn matches(&self, query: &str, source: Option<ModSource>) -> bool {
        if source.is_some_and(|expected| expected != self.source) {
            return false;
        }

        let query = query.trim().to_lowercase();
        query.is_empty() || self.search_text().contains(&query)
    }

    #[cfg(test)]
    pub(crate) fn test_record(mod_id: &str, name: &str, author: &str) -> Self {
        Self {
            root: PathBuf::from(mod_id),
            source: ModSource::Local,
            identity: ModIdentity {
                mod_id: mod_id.to_owned(),
                name: name.to_owned(),
                author: author.to_owned(),
                version: "1.0.0".to_owned(),
                description: "Baseline description".to_owned(),
                dependencies: Vec::new(),
            },
            display: DisplayMetadata::default(),
            enabled: false,
            assets: ModAssets::default(),
            profile: None,
            integration: None,
            dependency_summary: String::new(),
            dependency_health: DependencyHealth::default(),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_display(mut self, display: DisplayMetadata) -> Self {
        self.display = display;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_source(mut self, source: ModSource) -> Self {
        self.source = source;
        self
    }
}

impl From<&ModRecord> for SelectedModView {
    fn from(record: &ModRecord) -> Self {
        let integration = record.integration.as_ref();
        Self {
            name: record
                .display
                .title
                .clone()
                .unwrap_or_else(|| record.identity.name.clone()),
            author: record
                .display
                .author
                .clone()
                .unwrap_or_else(|| record.identity.author.clone()),
            version: record
                .display
                .version
                .clone()
                .unwrap_or_else(|| record.identity.version.clone()),
            description: record
                .display
                .summary
                .clone()
                .or_else(|| integration.map(|manifest| manifest.summary.clone()))
                .filter(|summary| !summary.is_empty())
                .unwrap_or_else(|| record.identity.description.clone()),
            source: record.source,
            enabled: record.enabled,
            dependency_summary: record.dependency_summary.clone(),
            dependency_health: record.dependency_health.clone(),
            assets: record.assets.clone(),
            profile: record.profile.clone(),
            controls: integration
                .map(|manifest| manifest.controls.clone())
                .unwrap_or_default(),
        }
    }
}

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
