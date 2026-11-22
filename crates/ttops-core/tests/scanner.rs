use std::sync::atomic::AtomicBool;

use ttops_core::{
    container::{ContainerImage, ContainerProvider},
    model::{Category, Rule, RuleKind, ScanStatus},
    platform::{Platform, TestPlatform},
    scanner::{NoopProgress, scan},
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

fn path_rule(kind: RuleKind, path: String) -> Rule {
    Rule {
        id: "cache".into(),
        name: "Cache".into(),
        category: Category::A,
        kind,
        paths: vec![path],
        regenerable: true,
        preselect: true,
        review_only: false,
        guidance: None,
        process_names: vec![],
        permanent_only: false,
        outputs: vec![],
    }
}

#[test]
fn scans_children_read_only_and_skips_missing_paths() {
    let temp = tempfile::tempdir().unwrap();
    let platform = TestPlatform::new(temp.path(), "");
    let cache = platform.cache_dir().join("entries");
    std::fs::create_dir_all(cache.join("first")).unwrap();
    std::fs::write(cache.join("first/file"), b"abc").unwrap();
    std::fs::write(cache.join("second"), b"four").unwrap();
    let before = std::fs::read(cache.join("first/file")).unwrap();
    let cancel = AtomicBool::new(false);
    let session = scan(
        &platform,
        &[
            path_rule(RuleKind::Children, cache.to_string_lossy().into_owned()),
            path_rule(
                RuleKind::Dir,
                platform
                    .cache_dir()
                    .join("missing")
                    .to_string_lossy()
                    .into_owned(),
            ),
        ],
        &NoContainers,
        &cancel,
        &NoopProgress,
    );
    assert_eq!(session.status, ScanStatus::Complete);
    assert_eq!(session.items.len(), 2);
    assert_eq!(
        session
            .items
            .iter()
            .map(|item| item.size_bytes)
            .sum::<u64>(),
        7
    );
    assert!(session.items.iter().all(|item| item.preselected));
    assert_eq!(std::fs::read(cache.join("first/file")).unwrap(), before);
    assert!(session.unscanned.is_empty());
}

#[test]
fn honours_a_preexisting_cancellation_request() {
    let temp = tempfile::tempdir().unwrap();
    let platform = TestPlatform::new(temp.path(), "");
    let cancel = AtomicBool::new(true);
    let session = scan(&platform, &[], &NoContainers, &cancel, &NoopProgress);
    assert_eq!(session.status, ScanStatus::Cancelled);
    assert!(session.items.is_empty());
}
