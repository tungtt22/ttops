use tempfile::TempDir;
use ttops_core::platform::TestPlatform;
use ttops_core::rules::{expand_paths, load_catalog, load_platform_catalog};

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
