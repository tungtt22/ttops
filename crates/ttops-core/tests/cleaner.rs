use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use ttops_core::{
    audit::AuditLog,
    cleaner::{Cleaner, CleanerError},
    container::{ContainerImage, ContainerProvider},
    model::{Category, Outcome, Rule, RuleKind, ScanSession, SkipReason},
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

fn session(platform: &TestPlatform) -> ScanSession {
    let path = platform.cache_dir().join("clean-me");
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join("file"), b"contents").unwrap();
    let rule = Rule {
        id: "cache".into(),
        name: "Cache".into(),
        category: Category::A,
        kind: RuleKind::Dir,
        paths: vec![path.to_string_lossy().into_owned()],
        regenerable: true,
        preselect: true,
        review_only: false,
        guidance: None,
        process_names: vec![],
        permanent_only: false,
        outputs: vec![],
    };
    scan(
        platform,
        &[rule],
        &NoContainers,
        &AtomicBool::new(false),
        &NoopProgress,
    )
}

#[test]
fn preview_is_side_effect_free_and_trash_run_is_single_use_and_audited() {
    let temp = tempfile::tempdir().unwrap();
    let platform = Arc::new(TestPlatform::new(temp.path(), ""));
    let scan = session(platform.as_ref());
    let item = scan.items[0].clone();
    let audit = AuditLog::new(temp.path().join("audit.jsonl"));
    let mut cleaner = Cleaner::new(platform.clone(), Arc::new(NoContainers), audit);
    let preview = cleaner.preview(&scan, std::slice::from_ref(&item.id), false);
    let path = match &item.target {
        ttops_core::model::Target::Path { path, .. } => path,
        _ => unreachable!(),
    };
    assert!(path.exists());
    let run = cleaner.run(&preview.id, false).unwrap();
    assert_eq!(run.freed_bytes, 8);
    assert!(!path.exists());
    assert!(platform.trash_dir().join("clean-me").exists());
    assert!(matches!(
        cleaner.run(&preview.id, false),
        Err(CleanerError::PreviewUsed)
    ));
}

#[test]
fn permanent_execution_requires_acknowledgement_without_modifying_the_target() {
    let temp = tempfile::tempdir().unwrap();
    let platform = Arc::new(TestPlatform::new(temp.path(), ""));
    let scan = session(platform.as_ref());
    let item = scan.items[0].clone();
    let path = match &item.target {
        ttops_core::model::Target::Path { path, .. } => path.clone(),
        _ => unreachable!(),
    };
    let mut cleaner = Cleaner::new(
        platform.clone(),
        Arc::new(NoContainers),
        AuditLog::new(temp.path().join("audit.jsonl")),
    );
    let preview = cleaner.preview(&scan, &[item.id], true);
    assert!(matches!(
        cleaner.run(&preview.id, false),
        Err(CleanerError::AckRequired)
    ));
    assert!(path.exists());
    cleaner.run(&preview.id, true).unwrap();
    assert!(!path.exists());
}

#[test]
fn dir_contents_skips_a_protected_child_and_continues_with_safe_siblings() {
    let temp = tempfile::tempdir().unwrap();
    let platform = Arc::new(TestPlatform::new(temp.path(), ""));
    let cache = platform.cache_dir().join("mixed-cache");
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::write(cache.join("safe"), b"safe").unwrap();
    std::fs::write(cache.join("credentials"), b"secret").unwrap();
    let scan = scan(
        platform.as_ref(),
        &[Rule {
            id: "mixed-cache".into(),
            name: "Mixed cache".into(),
            category: Category::A,
            kind: RuleKind::DirContents,
            paths: vec![cache.to_string_lossy().into_owned()],
            regenerable: true,
            preselect: true,
            review_only: false,
            guidance: None,
            process_names: vec![],
            permanent_only: false,
            outputs: vec![],
        }],
        &NoContainers,
        &AtomicBool::new(false),
        &NoopProgress,
    );
    let mut cleaner = Cleaner::new(
        platform.clone(),
        Arc::new(NoContainers),
        AuditLog::new(temp.path().join("audit.jsonl")),
    );
    let preview = cleaner.preview(&scan, &[scan.items[0].id.clone()], true);
    let run = cleaner.run(&preview.id, true).unwrap();

    assert!(!cache.join("safe").exists());
    assert!(cache.join("credentials").exists());
    assert!(run.results.iter().any(|result| {
        result.label == "credentials"
            && result.outcome == Outcome::Skipped
            && result.reason == Some(SkipReason::Protected)
    }));
    assert!(
        run.results
            .iter()
            .any(|result| { result.label == "safe" && result.outcome == Outcome::Succeeded })
    );
}
