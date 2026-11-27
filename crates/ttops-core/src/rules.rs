//! Data-driven cleanup rule catalogs and platform token expansion.

use std::collections::HashSet;
use std::path::PathBuf;

use serde::Deserialize;
use thiserror::Error;

use crate::model::{Category, Rule, RuleKind};
use crate::platform::Platform;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuleError {
    #[error("invalid catalog: {0}")]
    Invalid(String),
    #[error("TOML: {0}")]
    Toml(String),
}

#[derive(Deserialize)]
struct Catalog {
    #[serde(default)]
    rules: Vec<Rule>,
}

pub fn load_catalog(source: &str) -> Result<Vec<Rule>, RuleError> {
    let catalog: Catalog =
        toml::from_str(source).map_err(|error| RuleError::Toml(error.to_string()))?;
    validate(&catalog.rules)?;
    Ok(catalog.rules)
}

pub fn load_platform_catalog(platform: &dyn Platform) -> Result<Vec<Rule>, RuleError> {
    load_catalog(platform.catalog_source())
}

pub fn expand_paths(rule: &Rule, platform: &dyn Platform) -> Vec<PathBuf> {
    rule.paths
        .iter()
        .filter_map(|path| expand_path(path, platform))
        .collect()
}

fn expand_path(path: &str, platform: &dyn Platform) -> Option<PathBuf> {
    let mut expanded = path.replace("{cache}", &platform.cache_dir().to_string_lossy());
    expanded = expanded.replace("{data}", &platform.data_dir().to_string_lossy());
    expanded = expanded.replace("{local_data}", &platform.local_data_dir().to_string_lossy());
    if expanded == "~" || expanded.starts_with("~/") {
        expanded = format!("{}{}", platform.home().display(), &expanded[1..]);
    }
    if expanded.starts_with('%') {
        let end = expanded[1..].find('%')? + 1;
        let name = &expanded[1..end];
        expanded = format!("{}{}", platform.env_var(name)?, &expanded[end + 1..]);
    }
    Some(PathBuf::from(expanded))
}

fn validate(rules: &[Rule]) -> Result<(), RuleError> {
    let mut ids = HashSet::new();
    for rule in rules {
        if rule.id.is_empty()
            || !rule
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(RuleError::Invalid(format!(
                "id must be kebab-case: {}",
                rule.id
            )));
        }
        if !ids.insert(&rule.id) {
            return Err(RuleError::Invalid(format!("duplicate id: {}", rule.id)));
        }
        if rule.name.trim().is_empty() {
            return Err(RuleError::Invalid(format!("name required: {}", rule.id)));
        }
        if rule.preselect && !rule.regenerable {
            return Err(RuleError::Invalid(format!(
                "preselect requires regenerable: {}",
                rule.id
            )));
        }
        if rule.preselect && rule.category != Category::A {
            return Err(RuleError::Invalid(format!(
                "preselect must be false for category B and C: {}",
                rule.id
            )));
        }
        if rule.review_only
            && rule
                .guidance
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
        {
            return Err(RuleError::Invalid(format!(
                "guidance required when reviewOnly: {}",
                rule.id
            )));
        }
        if rule.kind == RuleKind::ProjectOutputs && rule.outputs.is_empty() {
            return Err(RuleError::Invalid(format!("outputs required: {}", rule.id)));
        }
        if matches!(
            rule.kind,
            RuleKind::Dir | RuleKind::DirContents | RuleKind::Children | RuleKind::LargeFiles
        ) && rule.paths.is_empty()
        {
            return Err(RuleError::Invalid(format!("paths required: {}", rule.id)));
        }
    }
    Ok(())
}
