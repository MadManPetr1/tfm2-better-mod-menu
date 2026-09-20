// SPDX-License-Identifier: GPL-3.0-or-later
// See LICENSE-EXCEPTION.md for the TFM2 linking exception and attribution terms.

use crate::model::{DependencyHealth, DependencyIssue, DependencyIssueKind, ModDependency};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug)]
pub(crate) struct InstalledDependency {
    pub display_name: String,
    pub version: String,
}

impl InstalledDependency {
    pub(crate) fn new(display_name: &str, version: &str) -> Self {
        Self {
            display_name: display_name.to_owned(),
            version: version.to_owned(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RequirementOperator {
    Equal,
    Greater,
    GreaterOrEqual,
    Less,
    LessOrEqual,
}

impl RequirementOperator {
    fn text(self) -> &'static str {
        match self {
            Self::Equal => "=",
            Self::Greater => ">",
            Self::GreaterOrEqual => ">=",
            Self::Less => "<",
            Self::LessOrEqual => "<=",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RequirementClause {
    pub operator: RequirementOperator,
    pub version: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RequirementError(pub String);

fn parse_version(version: &str) -> Result<Vec<u64>, RequirementError> {
    let version = version.trim().strip_prefix('v').unwrap_or(version.trim());
    if version.is_empty() {
        return Err(RequirementError("version is empty".to_owned()));
    }
    version
        .split('.')
        .map(|component| {
            if component.is_empty() || !component.chars().all(|value| value.is_ascii_digit()) {
                return Err(RequirementError(format!(
                    "version component {component:?} is not numeric"
                )));
            }
            component
                .parse::<u64>()
                .map_err(|error| RequirementError(format!("invalid version: {error}")))
        })
        .collect()
}

pub(crate) fn compare_versions(left: &str, right: &str) -> Result<Ordering, RequirementError> {
    let mut left = parse_version(left)?;
    let mut right = parse_version(right)?;
    let length = left.len().max(right.len());
    left.resize(length, 0);
    right.resize(length, 0);
    Ok(left.cmp(&right))
}

pub(crate) fn parse_requirement(
    requirement: &str,
) -> Result<Vec<RequirementClause>, RequirementError> {
    if requirement.trim().is_empty() {
        return Err(RequirementError("requirement is empty".to_owned()));
    }

    requirement
        .split(',')
        .map(|raw_clause| {
            let clause = raw_clause.trim();
            let (operator, version) = if let Some(version) = clause.strip_prefix(">=") {
                (RequirementOperator::GreaterOrEqual, version)
            } else if let Some(version) = clause.strip_prefix("<=") {
                (RequirementOperator::LessOrEqual, version)
            } else if let Some(version) = clause.strip_prefix('=') {
                (RequirementOperator::Equal, version)
            } else if let Some(version) = clause.strip_prefix('>') {
                (RequirementOperator::Greater, version)
            } else if let Some(version) = clause.strip_prefix('<') {
                (RequirementOperator::Less, version)
            } else {
                return Err(RequirementError(format!(
                    "dependency clause {clause:?} has no supported operator"
                )));
            };
            let version = version.trim();
            parse_version(version)?;
            Ok(RequirementClause {
                operator,
                version: version.to_owned(),
            })
        })
        .collect()
}

pub(crate) fn version_satisfies(
    version: &str,
    requirement: &str,
) -> Result<bool, RequirementError> {
    parse_version(version)?;
    let clauses = parse_requirement(requirement)?;
    clauses.into_iter().try_fold(true, |matches, clause| {
        let ordering = compare_versions(version, &clause.version)?;
        let clause_matches = match clause.operator {
            RequirementOperator::Equal => ordering == Ordering::Equal,
            RequirementOperator::Greater => ordering == Ordering::Greater,
            RequirementOperator::GreaterOrEqual => ordering != Ordering::Less,
            RequirementOperator::Less => ordering == Ordering::Less,
            RequirementOperator::LessOrEqual => ordering != Ordering::Greater,
        };
        Ok(matches && clause_matches)
    })
}

pub(crate) fn friendly_requirement(requirement: &str) -> Result<String, RequirementError> {
    Ok(parse_requirement(requirement)?
        .into_iter()
        .map(|clause| format!("{} {}", clause.operator.text(), clause.version))
        .collect::<Vec<_>>()
        .join(" and "))
}

pub(crate) fn friendly_dependency_text(
    dependency: &ModDependency,
    display_name: Option<&str>,
) -> String {
    let requirement = friendly_requirement(&dependency.version)
        .unwrap_or_else(|_| dependency.version.trim().to_owned());
    if dependency.mod_id == "base" {
        format!("TFM2 version {requirement}")
    } else {
        format!(
            "Requires {} {requirement}",
            display_name.unwrap_or(&dependency.mod_id)
        )
    }
}

pub(crate) fn evaluate_dependencies(
    dependencies: &[ModDependency],
    installed: &HashMap<String, InstalledDependency>,
    enabled: &HashSet<String>,
    game_version: &str,
) -> DependencyHealth {
    let mut issues = Vec::new();
    for dependency in dependencies {
        let requirement = match parse_requirement(&dependency.version) {
            Ok(requirement) => requirement,
            Err(_) => {
                issues.push(DependencyIssue {
                    kind: DependencyIssueKind::Malformed,
                    message: format!(
                        "Dependency {} has malformed requirement {}.",
                        dependency.mod_id, dependency.version
                    ),
                });
                continue;
            }
        };
        let normalized_requirement = requirement
            .iter()
            .map(|clause| format!("{} {}", clause.operator.text(), clause.version))
            .collect::<Vec<_>>()
            .join(" and ");

        if dependency.mod_id == "base" {
            match version_satisfies(game_version, &dependency.version) {
                Ok(true) => {}
                Ok(false) => issues.push(DependencyIssue {
                    kind: DependencyIssueKind::Incompatible,
                    message: format!(
                        "TFM2 {game_version} does not satisfy {normalized_requirement}."
                    ),
                }),
                Err(_) => issues.push(DependencyIssue {
                    kind: DependencyIssueKind::Malformed,
                    message: format!(
                        "TFM2 dependency has malformed requirement {}.",
                        dependency.version
                    ),
                }),
            }
            continue;
        }

        let Some(installed_dependency) = installed.get(&dependency.mod_id) else {
            issues.push(DependencyIssue {
                kind: DependencyIssueKind::Missing,
                message: format!(
                    "Requires {} {normalized_requirement}, but it is not installed.",
                    dependency.mod_id
                ),
            });
            continue;
        };
        if !enabled.contains(&dependency.mod_id) {
            issues.push(DependencyIssue {
                kind: DependencyIssueKind::Disabled,
                message: format!(
                    "{} is installed but disabled.",
                    installed_dependency.display_name
                ),
            });
            continue;
        }
        match version_satisfies(&installed_dependency.version, &dependency.version) {
            Ok(true) => {}
            Ok(false) => issues.push(DependencyIssue {
                kind: DependencyIssueKind::Incompatible,
                message: format!(
                    "{} {} does not satisfy {normalized_requirement}.",
                    installed_dependency.display_name, installed_dependency.version
                ),
            }),
            Err(_) => issues.push(DependencyIssue {
                kind: DependencyIssueKind::Malformed,
                message: format!(
                    "{} has an unreadable installed version {}.",
                    installed_dependency.display_name, installed_dependency.version
                ),
            }),
        }
    }

    if issues.is_empty() {
        DependencyHealth {
            warning: false,
            tooltip: "Dependencies are installed, enabled, and compatible.".to_owned(),
            issues,
        }
    } else {
        DependencyHealth {
            warning: true,
            tooltip: issues
                .iter()
                .map(|issue| issue.message.as_str())
                .collect::<Vec<_>>()
                .join("  |  "),
            issues,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ModDependency;
    use std::collections::{HashMap, HashSet};

    fn dependency(mod_id: &str, version: &str) -> ModDependency {
        ModDependency {
            mod_id: mod_id.to_owned(),
            version: version.to_owned(),
        }
    }

    #[test]
    fn evaluates_supported_multi_clause_requirements() {
        assert!(version_satisfies("0.6.0", ">=0.6.0").unwrap());
        assert!(version_satisfies("0.5.3", ">=0.5.2, <0.5.4").unwrap());
        assert!(!version_satisfies("0.5.4", ">=0.5.2, <0.5.4").unwrap());
    }

    #[test]
    fn malformed_clause_becomes_a_dependency_warning() {
        let dependencies = vec![ModDependency {
            mod_id: "base".to_owned(),
            version: ">>0.6".to_owned(),
        }];
        let health =
            evaluate_dependencies(&dependencies, &HashMap::new(), &HashSet::new(), "0.6.0");
        assert!(health.warning);
        assert!(health.tooltip.contains(">>0.6"));
    }

    #[test]
    fn reports_missing_disabled_and_incompatible_mods() {
        let installed = HashMap::from([
            (
                "disabled".to_owned(),
                InstalledDependency::new("Disabled Mod", "1.0.0"),
            ),
            (
                "old".to_owned(),
                InstalledDependency::new("Old Mod", "1.0.0"),
            ),
        ]);
        let dependencies = vec![
            dependency("missing", ">=1.0.0"),
            dependency("disabled", ">=1.0.0"),
            dependency("old", ">=2.0.0"),
        ];
        let enabled = HashSet::from(["old".to_owned()]);
        let health = evaluate_dependencies(&dependencies, &installed, &enabled, "0.6.0");
        assert_eq!(health.issues.len(), 3);
        assert_eq!(health.issues[0].kind, DependencyIssueKind::Missing);
        assert_eq!(health.issues[1].kind, DependencyIssueKind::Disabled);
        assert_eq!(health.issues[2].kind, DependencyIssueKind::Incompatible);
    }

    #[test]
    fn compares_equality_bounds_prefixes_and_uneven_versions() {
        assert!(version_satisfies("v1.2", "=1.2.0").unwrap());
        assert!(version_satisfies("1.2.1", ">1.2").unwrap());
        assert!(version_satisfies("1.2", "<=1.2.0").unwrap());
        assert!(!version_satisfies("1.1.9", ">=1.2").unwrap());
    }

    #[test]
    fn friendly_text_names_base_and_installed_mods() {
        assert_eq!(
            friendly_dependency_text(&dependency("base", ">=0.5.2, <0.5.4"), None),
            "TFM2 version >= 0.5.2 and < 0.5.4"
        );
        assert_eq!(
            friendly_dependency_text(&dependency("intro_skip", ">=0.4.0"), Some("Intro Skip")),
            "Requires Intro Skip >= 0.4.0"
        );
    }

    #[test]
    fn healthy_set_has_stable_success_tooltip() {
        let installed = HashMap::from([(
            "other".to_owned(),
            InstalledDependency::new("Other", "2.0.0"),
        )]);
        let enabled = HashSet::from(["other".to_owned()]);
        let health = evaluate_dependencies(
            &[dependency("base", ">=0.6.0"), dependency("other", "=2.0.0")],
            &installed,
            &enabled,
            "0.6.1",
        );
        assert!(!health.warning);
        assert!(health.issues.is_empty());
        assert_eq!(
            health.tooltip,
            "Dependencies are installed, enabled, and compatible."
        );
    }
}
