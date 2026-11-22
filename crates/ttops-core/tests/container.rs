use std::sync::atomic::AtomicBool;

use ttops_core::{
    container::{ContainerImage, ContainerProvider},
    model::{Category, Rule, RuleKind, ScanStatus},
    platform::TestPlatform,
    scanner::{NoopProgress, scan},
};

struct FakeContainer {
    available: bool,
}
impl ContainerProvider for FakeContainer {
    fn available(&self) -> bool {
        self.available
    }
    fn build_cache_bytes(&self) -> Result<u64, String> {
        Ok(7)
    }
    fn unused_images(&self) -> Result<Vec<ContainerImage>, String> {
        Ok(vec![ContainerImage {
            id: "image-id".into(),
            repo_tag: "registry/image:tag".into(),
            size_bytes: 11,
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

fn rule(id: &str, kind: RuleKind) -> Rule {
    Rule {
        id: id.into(),
        name: id.into(),
        category: Category::A,
        kind,
        paths: vec![],
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
fn reports_container_items_when_provider_is_available() {
    let temp = tempfile::tempdir().unwrap();
    let platform = TestPlatform::new(temp.path(), "");
    let cancel = AtomicBool::new(false);
    let session = scan(
        &platform,
        &[
            rule("build", RuleKind::ContainerBuildCache),
            rule("images", RuleKind::ContainerImages),
        ],
        &FakeContainer { available: true },
        &cancel,
        &NoopProgress,
    );
    assert_eq!(session.status, ScanStatus::Complete);
    assert!(session.container_available);
    assert_eq!(session.items.len(), 2);
    assert_eq!(
        session
            .items
            .iter()
            .map(|item| item.size_bytes)
            .sum::<u64>(),
        18
    );
}

#[test]
fn unavailable_container_provider_is_not_an_error() {
    let temp = tempfile::tempdir().unwrap();
    let platform = TestPlatform::new(temp.path(), "");
    let cancel = AtomicBool::new(false);
    let session = scan(
        &platform,
        &[rule("build", RuleKind::ContainerBuildCache)],
        &FakeContainer { available: false },
        &cancel,
        &NoopProgress,
    );
    assert!(!session.container_available);
    assert!(session.items.is_empty());
}
