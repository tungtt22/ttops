use std::sync::atomic::AtomicBool;

use tempfile::TempDir;
use ttops_core::{
    container::{ContainerImage, ContainerProvider},
    model::{Category, RuleKind, Target},
    platform::{Platform, TestPlatform},
    rules::{expand_paths, load_catalog, load_platform_catalog},
    scanner::{NoopProgress, scan},
};

struct FakeContainer;
impl ContainerProvider for FakeContainer {
    fn available(&self) -> bool {
        true
    }
    fn build_cache_bytes(&self) -> Result<u64, String> {
        Ok(13)
    }
    fn unused_images(&self) -> Result<Vec<ContainerImage>, String> {
        Ok(vec![ContainerImage {
            id: "unused".into(),
            repo_tag: "registry/image:latest".into(),
            size_bytes: 17,
            used: false,
            locally_built: false,
        }])
    }
    fn prune_build_cache(&self) -> Result<(), String> {
        Ok(())
    }
    fn remove_image(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
}

const VALID: &str = r#"
[[rules]]
id = "cache"
name = "Cache"
category = "A"
kind = "dir"
paths = ["{cache}/npm", "~/extra", "%TTOPS_TEST%/x"]
regenerable = true
preselect = true
"#;

#[test]
fn expands_tokens_and_loads_catalogs() {
    let temp = TempDir::new().unwrap();
    let platform = TestPlatform::new(temp.path(), VALID);
    let rules = load_platform_catalog(&platform).unwrap();
    assert_eq!(expand_paths(&rules[0], &platform).len(), 3);
    assert!(load_catalog(include_str!("../rules/macos.toml")).is_ok());
    assert!(load_catalog(include_str!("../rules/windows.toml")).is_ok());
}

#[test]
fn rejects_unsafe_catalogs() {
    let invalid = VALID.replace("regenerable = true", "regenerable = false");
    assert!(
        load_catalog(&invalid)
            .unwrap_err()
            .to_string()
            .contains("preselect requires")
    );
    let invalid = VALID.replace("category = \"A\"", "category = \"C\"");
    assert!(
        load_catalog(&invalid)
            .unwrap_err()
            .to_string()
            .contains("must be false")
    );
    let invalid = VALID.replace("id = \"cache\"", "id = \"cache\"\n\n[[rules]]\nid = \"cache\"\nname = \"Other\"\ncategory = \"A\"\nkind = \"dir\"\npaths = [\"~/x\"]\nregenerable = true");
    assert!(load_catalog(&invalid).is_err());
}

#[test]
fn macos_category_a_catalog_expands_platform_locations_and_scans_seeded_cache_and_container_items()
{
    let temp = TempDir::new().unwrap();
    let platform = TestPlatform::new(temp.path(), include_str!("../rules/macos.toml"));
    let rules = load_platform_catalog(&platform).unwrap();
    let ids: Vec<_> = rules.iter().map(|rule| rule.id.as_str()).collect();
    for required in [
        "browser-caches",
        "npm-cache",
        "pip-cache",
        "uv-cache",
        "homebrew-cache",
        "poetry-cache",
        "gradle-cache",
        "maven-cache",
        "cargo-registry",
        "app-updater-caches",
        "container-build-cache",
        "container-unused-images",
        "trash",
    ] {
        assert!(ids.contains(&required), "missing {required}");
    }
    assert!(rules.iter().all(|rule| rule.category == Category::A));
    assert!(
        rules
            .iter()
            .flat_map(|rule| rule.paths.iter())
            .all(|path| !path.contains("/Users/"))
    );

    std::fs::create_dir_all(platform.home().join(".npm")).unwrap();
    std::fs::write(platform.home().join(".npm/cache"), b"home-npm").unwrap();
    std::fs::create_dir_all(platform.home().join(".Trash")).unwrap();
    std::fs::write(platform.home().join(".Trash/item"), b"trash").unwrap();

    let session = scan(
        &platform,
        &rules,
        &FakeContainer,
        &AtomicBool::new(false),
        &NoopProgress,
    );
    assert!(session.items.iter().any(|item| item.rule_id == "npm-cache"));
    assert!(session.items.iter().any(|item| item.rule_id == "trash"));
    assert!(
        session
            .items
            .iter()
            .any(|item| matches!(item.target, Target::ContainerBuildCache))
    );
    assert!(
        session
            .items
            .iter()
            .any(|item| matches!(item.target, Target::ContainerImage { .. }))
    );
    assert!(
        rules
            .iter()
            .any(|rule| rule.kind == RuleKind::ContainerImages)
    );
}
