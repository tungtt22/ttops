//! Category B (project build outputs) and C (review-only large data) scanning.

use std::path::Path;
use std::sync::atomic::AtomicBool;

use ttops_core::{
    container::{ContainerImage, ContainerProvider},
    guard::Guard,
    model::{Category, OutputPattern, Rule, RuleKind, ScanStatus, Settings, Target},
    platform::{Platform, TestPlatform},
    scanner::{NoopProgress, scan_with_settings},
    settings::validate,
};

struct NoContainers;
impl ContainerProvider for NoContainers {
    fn available(&self) -> bool {
        false
    }
    fn build_cache_bytes(&self) -> Result<u64, String> {
        Ok(0)
    }
    fn unused_images(&self) -> Result<Vec<ContainerImage>, String> {
        Ok(vec![])
    }
    fn prune_build_cache(&self) -> Result<(), String> {
        Ok(())
    }
    fn remove_image(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
}

fn rule(id: &str, category: Category, kind: RuleKind) -> Rule {
    Rule {
        id: id.into(),
        name: id.into(),
        category,
        kind,
        paths: vec![],
        regenerable: category != Category::C,
        preselect: false,
        review_only: false,
        guidance: None,
        process_names: vec![],
        permanent_only: false,
        outputs: vec![],
    }
}

fn outputs_rule() -> Rule {
    Rule {
        outputs: vec![
            OutputPattern {
                name: "node_modules".into(),
                markers: vec!["package.json".into()],
            },
            OutputPattern {
                name: "target".into(),
                markers: vec!["Cargo.toml".into()],
            },
        ],
        ..rule("project-outputs", Category::B, RuleKind::ProjectOutputs)
    }
}

fn write(path: &Path, size: usize) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, vec![b'x'; size]).unwrap();
}

fn run(
    platform: &TestPlatform,
    rules: &[Rule],
    settings: &Settings,
) -> ttops_core::model::ScanSession {
    scan_with_settings(
        "scan-test".into(),
        platform,
        rules,
        settings,
        &NoContainers,
        &AtomicBool::new(false),
        &NoopProgress,
    )
}

fn target_path(item: &ttops_core::model::ScanItem) -> &Path {
    match &item.target {
        Target::Path { path, .. } => path,
        other => panic!("unexpected target {other:?}"),
    }
}

#[test]
fn project_outputs_are_outermost_unselected_and_carry_project_age() {
    let temp = tempfile::tempdir().unwrap();
    let platform = TestPlatform::new(temp.path(), "");
    let root = platform.home().join("code");
    let web = root.join("web");
    write(&web.join("package.json"), 2);
    write(&web.join("node_modules/a/index.js"), 10);
    // Nested output inside an output: must not be listed separately.
    write(&web.join("node_modules/a/package.json"), 1);
    write(&web.join("node_modules/a/node_modules/b/index.js"), 5);
    let rust = root.join("tool");
    write(&rust.join("Cargo.toml"), 1);
    write(&rust.join("target/debug/bin"), 20);
    // A `target` folder without its marker is not a build output.
    write(&root.join("notes/target/plan.txt"), 3);

    let settings = Settings {
        project_roots: vec![root.clone()],
        ..Settings::default()
    };
    let session = run(&platform, &[outputs_rule()], &settings);

    assert_eq!(session.status, ScanStatus::Complete);
    let mut paths: Vec<_> = session.items.iter().map(target_path).collect();
    paths.sort();
    assert_eq!(paths, vec![rust.join("target"), web.join("node_modules")]);
    let web_item = session
        .items
        .iter()
        .find(|item| target_path(item) == web.join("node_modules"))
        .unwrap();
    assert_eq!(web_item.size_bytes, 16);
    assert_eq!(web_item.category, Category::B);
    assert_eq!(web_item.project_path.as_deref(), Some(web.as_path()));
    assert_eq!(web_item.project_age_days, Some(0));
    assert!(web_item.selectable);
    assert!(session.items.iter().all(|item| !item.preselected));
    assert_eq!(
        std::fs::metadata(web.join("node_modules/a/index.js"))
            .unwrap()
            .len(),
        10
    );
}

#[test]
fn no_project_roots_yields_no_category_b_items() {
    let temp = tempfile::tempdir().unwrap();
    let platform = TestPlatform::new(temp.path(), "");
    write(&platform.home().join("web/package.json"), 1);
    write(&platform.home().join("web/node_modules/x"), 1);
    let session = run(&platform, &[outputs_rule()], &Settings::default());
    assert!(session.items.is_empty());
}

#[test]
fn large_files_respect_threshold_claims_and_protection() {
    let temp = tempfile::tempdir().unwrap();
    let platform = TestPlatform::new(temp.path(), "");
    let home = platform.home();
    write(&home.join("videos/big.mov"), 64);
    write(&home.join("videos/small.mov"), 8);
    write(&home.join("models/llm.bin"), 128);
    write(&home.join("keys/server.pem"), 64);

    let mut models = rule("model-stores", Category::C, RuleKind::Dir);
    models.paths = vec![home.join("models").to_string_lossy().into_owned()];
    let mut large = rule("large-files", Category::C, RuleKind::LargeFiles);
    large.paths = vec!["~".into()];
    let settings = Settings {
        large_file_threshold_bytes: 32,
        ..Settings::default()
    };
    // Large files are listed first in the catalog but must still skip claimed paths.
    let session = run(&platform, &[large, models], &settings);

    let large_paths: Vec<_> = session
        .items
        .iter()
        .filter(|item| item.rule_id == "large-files")
        .map(target_path)
        .collect();
    assert_eq!(large_paths, vec![home.join("videos/big.mov")]);
    assert!(session.items.iter().all(|item| !item.preselected));
    assert!(
        session
            .items
            .iter()
            .all(|item| item.category == Category::C)
    );
}

#[test]
fn review_only_items_are_not_selectable_and_carry_guidance() {
    let temp = tempfile::tempdir().unwrap();
    let platform = TestPlatform::new(temp.path(), "");
    let media = platform.home().join("chat/media");
    write(&media.join("photo.jpg"), 4);
    let mut chat = rule("chat-media", Category::C, RuleKind::Dir);
    chat.paths = vec![media.to_string_lossy().into_owned()];
    chat.review_only = true;
    chat.guidance = Some("Clean inside the app".into());

    let session = run(&platform, &[chat], &Settings::default());

    assert_eq!(session.items.len(), 1);
    assert!(!session.items[0].selectable);
    assert!(!session.items[0].preselected);
    assert_eq!(
        session.items[0].guidance.as_deref(),
        Some("Clean inside the app")
    );
}

#[test]
fn settings_validation_rejects_protected_missing_and_file_roots() {
    let temp = tempfile::tempdir().unwrap();
    let platform = TestPlatform::new(temp.path(), "");
    let guard = Guard::new(&platform);
    let code = platform.home().join("code");
    std::fs::create_dir_all(&code).unwrap();
    write(&platform.home().join("file.txt"), 1);

    let ok = validate(
        Settings {
            project_roots: vec![code.clone(), code.clone()],
            large_file_threshold_bytes: 1 << 30,
        },
        &guard,
    )
    .unwrap();
    assert_eq!(ok.project_roots.len(), 1);

    for bad in [
        platform.home(),
        platform.home().join("missing"),
        platform.home().join("file.txt"),
    ] {
        let result = validate(
            Settings {
                project_roots: vec![bad.clone()],
                ..Settings::default()
            },
            &guard,
        );
        assert!(result.is_err(), "{bad:?} should be rejected");
    }
    assert!(
        validate(
            Settings {
                large_file_threshold_bytes: 0,
                ..Settings::default()
            },
            &guard
        )
        .is_err()
    );
}
