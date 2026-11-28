//! Feature 002: app uninstaller, toolchain cleanup, duplicates, idle files, startup items.
//! Every test runs on a temp-dir `TestPlatform` with fake providers.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use ttops_core::{
    apps::scan_apps,
    audit::AuditLog,
    cleaner::Cleaner,
    container::{ContainerImage, ContainerProvider, StoppedContainer},
    guard::Guard,
    model::{
        Action, Category, Outcome, RefusalReason, Rule, RuleKind, ScanSession, Settings,
        SkipReason, StartupScope, Target, ToolCommand,
    },
    platform::{Platform, TestPlatform},
    scanner::{NoopProgress, scan_with_tools},
    startup::{self, LaunchControl, StartupError},
    tools::{NoTools, ToolProvider, parse_brew_autoremove, parse_brew_cleanup},
};

// ---------- helpers ----------

fn write(path: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn make_app(platform: &TestPlatform, file: &str, id: &str, exe: &str) -> PathBuf {
    let app = platform.app_dirs()[0].join(file);
    write(
        &app.join("Contents/Info.plist"),
        format!(
            r#"{{"CFBundleIdentifier":"{id}","CFBundleName":"{exe}","CFBundleExecutable":"{exe}"}}"#
        )
        .as_bytes(),
    );
    write(&app.join(format!("Contents/MacOS/{exe}")), &[0; 100]);
    app
}

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

fn cleaner(platform: TestPlatform, tools: Arc<dyn ToolProvider>) -> (Cleaner, PathBuf) {
    let audit = platform.root.join("audit.jsonl");
    let platform: Arc<dyn Platform> = Arc::new(platform);
    (
        Cleaner::new(platform, Arc::new(NoContainers), AuditLog::new(&audit)).with_tools(tools),
        audit,
    )
}

fn rule(id: &str, category: Category, kind: RuleKind) -> Rule {
    Rule {
        id: id.into(),
        name: id.into(),
        category,
        kind,
        paths: vec![],
        regenerable: false,
        preselect: false,
        review_only: false,
        guidance: None,
        process_names: vec![],
        permanent_only: false,
        outputs: vec![],
    }
}

fn scan(
    platform: &TestPlatform,
    rules: &[Rule],
    settings: &Settings,
    tools: &dyn ToolProvider,
) -> ScanSession {
    scan_with_tools(
        "scan".into(),
        platform,
        rules,
        settings,
        &NoContainers,
        tools,
        &AtomicBool::new(false),
        &NoopProgress,
    )
}

// ---------- US1: app uninstaller ----------

#[test]
fn app_scan_lists_bundles_with_exact_leftovers_and_marks_apple_apps() {
    let temp = tempfile::tempdir().unwrap();
    let platform = TestPlatform::new(temp.path(), "");
    let lib = platform.library_dir().unwrap();
    make_app(&platform, "Foo.app", "com.example.foo", "Foo");
    make_app(&platform, "Safari.app", "com.apple.Safari", "Safari");
    write(&lib.join("Caches/com.example.foo/blob"), &[0; 10]);
    write(&lib.join("Preferences/com.example.foo.plist"), &[0; 5]);
    write(
        &lib.join("Group Containers/TEAM1.com.example.foo/x"),
        &[0; 5],
    );
    write(
        &lib.join("Saved Application State/com.example.foo.savedState/s"),
        &[0; 5],
    );
    // Similar but different app: must not be a leftover of Foo.
    write(&lib.join("Caches/com.example.foobar/blob"), &[0; 10]);
    // Orphan of an app that is no longer installed.
    write(&lib.join("Containers/com.gone.tool/data"), &[0; 7]);
    write(&lib.join("Containers/com.apple.mail/data"), &[0; 7]);

    let session = scan_apps(&platform, &AtomicBool::new(false));

    let foo = session.items.iter().find(|i| i.label == "Foo").unwrap();
    let Target::App {
        leftovers,
        bundle_id,
        ..
    } = &foo.target
    else {
        panic!()
    };
    assert_eq!(bundle_id, "com.example.foo");
    let mut names: Vec<_> = leftovers
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec![
            "TEAM1.com.example.foo",
            "com.example.foo",
            "com.example.foo.plist",
            "com.example.foo.savedState"
        ]
    );
    assert!(foo.selectable && !foo.preselected);
    // Group Containers are privacy-guarded: listed, but never opened to be sized.
    assert_eq!(foo.size_bytes, 100 + foo_plist_len() + 10 + 5 + 5);

    let safari = session.items.iter().find(|i| i.label == "Safari").unwrap();
    assert!(!safari.selectable);

    let orphans: Vec<_> = session
        .items
        .iter()
        .filter(|i| i.rule_id == "app-orphans")
        .map(|i| i.label.clone())
        .collect();
    assert!(orphans.iter().any(|l| l.contains("com.gone.tool")));
    let guarded = session
        .items
        .iter()
        .find(|i| i.label.ends_with("com.gone.tool"))
        .unwrap();
    assert_eq!(guarded.size_bytes, 0);
    assert!(
        guarded
            .guidance
            .as_deref()
            .unwrap()
            .contains("Size not measured")
    );
    assert!(orphans.iter().any(|l| l.ends_with("com.example.foobar")));
    assert!(orphans.iter().all(|l| !l.contains("com.apple.")));
    assert!(
        orphans
            .iter()
            .all(|l| !l.ends_with("/com.example.foo") && !l.ends_with("com.example.foo.plist"))
    );
    assert!(session.items.iter().all(|i| !i.preselected));
}

fn foo_plist_len() -> u64 {
    r#"{"CFBundleIdentifier":"com.example.foo","CFBundleName":"Foo","CFBundleExecutable":"Foo"}"#
        .len() as u64
}

#[test]
fn uninstall_moves_bundle_and_leftovers_to_trash_even_when_permanent_requested() {
    let temp = tempfile::tempdir().unwrap();
    let platform = TestPlatform::new(temp.path(), "");
    let lib = platform.library_dir().unwrap();
    let app = make_app(&platform, "Foo.app", "com.example.foo", "Foo");
    write(&lib.join("Caches/com.example.foo/blob"), &[0; 10]);
    let trash = platform.trash_dir();
    let session = scan_apps(&platform, &AtomicBool::new(false));
    let id = session
        .items
        .iter()
        .find(|i| i.label == "Foo")
        .unwrap()
        .id
        .clone();

    let (mut cleaner, audit) = cleaner(platform, Arc::new(NoTools));
    let preview = cleaner.preview(&session, &[id], true);
    assert_eq!(preview.items[0].action, Action::Trash);
    assert!(!preview.requires_irreversible_ack);
    let run = cleaner.run(&preview.id, false).unwrap();

    assert!(
        run.results.iter().all(|r| r.outcome == Outcome::Succeeded),
        "{:?}",
        run.results
    );
    assert!(!app.exists() && !lib.join("Caches/com.example.foo").exists());
    assert!(trash.join("Foo.app").exists() && trash.join("com.example.foo").exists());
    assert!(std::fs::read_to_string(audit).unwrap().contains("Foo.app"));
}

#[test]
fn running_or_apple_apps_are_refused_and_untouched() {
    let temp = tempfile::tempdir().unwrap();
    let mut platform = TestPlatform::new(temp.path(), "");
    let app = make_app(&platform, "Foo.app", "com.example.foo", "Foo");
    let apple = make_app(&platform, "Notes.app", "com.apple.Notes", "Notes");
    let session = scan_apps(&platform, &AtomicBool::new(false));
    let ids: Vec<_> = session.items.iter().map(|i| i.id.clone()).collect();
    platform.processes = vec!["foo".into()];

    let (mut cleaner, _) = cleaner(platform, Arc::new(NoTools));
    let preview = cleaner.preview(&session, &ids, false);
    assert!(preview.items.is_empty());
    assert!(
        preview
            .refused
            .iter()
            .any(|r| r.reason == RefusalReason::AppNotRemovable)
    );
    assert!(
        preview
            .refused
            .iter()
            .any(|r| r.reason == RefusalReason::NotSelectable)
    );
    assert!(app.exists() && apple.exists());
}

#[test]
fn app_started_after_preview_is_skipped_as_in_use() {
    let temp = tempfile::tempdir().unwrap();
    let platform = TestPlatform::new(temp.path(), "");
    let app = make_app(&platform, "Foo.app", "com.example.foo", "Foo");
    let session = scan_apps(&platform, &AtomicBool::new(false));
    let id = session.items[0].id.clone();
    let shared = Arc::new(Mutex::new(platform));
    // Simulate the app launching between preview and run.
    struct Live(Arc<Mutex<TestPlatform>>);
    impl Platform for Live {
        fn home(&self) -> PathBuf {
            self.0.lock().unwrap().home()
        }
        fn cache_dir(&self) -> PathBuf {
            self.0.lock().unwrap().cache_dir()
        }
        fn data_dir(&self) -> PathBuf {
            self.0.lock().unwrap().data_dir()
        }
        fn local_data_dir(&self) -> PathBuf {
            self.0.lock().unwrap().local_data_dir()
        }
        fn env_var(&self, n: &str) -> Option<String> {
            self.0.lock().unwrap().env_var(n)
        }
        fn protected_roots(&self) -> Vec<PathBuf> {
            self.0.lock().unwrap().protected_roots()
        }
        fn catalog_source(&self) -> &'static str {
            ""
        }
        fn move_to_trash(&self, p: &[PathBuf]) -> Result<(), trash::Error> {
            self.0.lock().unwrap().move_to_trash(p)
        }
        fn running_processes(&self) -> Vec<String> {
            self.0.lock().unwrap().processes.clone()
        }
        fn volume_summary(&self) -> Option<ttops_core::model::VolumeSummary> {
            None
        }
        fn app_dirs(&self) -> Vec<PathBuf> {
            self.0.lock().unwrap().app_dirs()
        }
        fn library_dir(&self) -> Option<PathBuf> {
            self.0.lock().unwrap().library_dir()
        }
    }
    let audit = temp.path().join("audit.jsonl");
    let mut cleaner = Cleaner::new(
        Arc::new(Live(shared.clone())),
        Arc::new(NoContainers),
        AuditLog::new(audit),
    );
    let preview = cleaner.preview(&session, &[id], false);
    shared.lock().unwrap().processes = vec!["foo".into()];
    let run = cleaner.run(&preview.id, false).unwrap();
    assert_eq!(run.results[0].reason, Some(SkipReason::InUse));
    assert!(app.exists());
}

#[test]
fn guard_accepts_app_bundles_only_through_the_uninstaller_exception() {
    let temp = tempfile::tempdir().unwrap();
    let platform = TestPlatform::new(temp.path(), "");
    let app = make_app(&platform, "Foo.app", "com.example.foo", "Foo");
    let guard = Guard::new(&platform);
    let app_dirs = platform.app_dirs();
    // Regression: the general guard still refuses the bundle.
    assert!(guard.check(&app).is_err());
    assert!(guard.check_app_bundle(&app, &app_dirs).is_ok());
    // Not a direct child, not a bundle, a symlink, or the folder itself: refused.
    let nested = app_dirs[0].join("Sub/Bar.app");
    std::fs::create_dir_all(&nested).unwrap();
    assert!(guard.check_app_bundle(&nested, &app_dirs).is_err());
    assert!(guard.check_app_bundle(&app_dirs[0], &app_dirs).is_err());
    let plain = app_dirs[0].join("notes");
    std::fs::create_dir_all(&plain).unwrap();
    assert!(guard.check_app_bundle(&plain, &app_dirs).is_err());
    #[cfg(unix)]
    {
        let link = app_dirs[0].join("Link.app");
        std::os::unix::fs::symlink(&app, &link).unwrap();
        assert!(guard.check_app_bundle(&link, &app_dirs).is_err());
    }
    // Outside an applications folder.
    let elsewhere = platform.home().join("Downloads/Foo.app");
    std::fs::create_dir_all(&elsewhere).unwrap();
    assert!(guard.check_app_bundle(&elsewhere, &app_dirs).is_err());
}

// ---------- US2: toolchains ----------

#[test]
fn brew_dry_run_output_is_parsed() {
    let cleanup = "Would remove: /opt/homebrew/Cellar/foo/1.0 (12 files, 3.4MB)\nWould remove: /cache/bar.tar.gz (1.1MB)\n==> This operation would free approximately 1.5GB of disk space.\n";
    let (bytes, lines) = parse_brew_cleanup(cleanup);
    assert_eq!(bytes, (1.5 * (1u64 << 30) as f64) as u64);
    assert_eq!(lines.len(), 2);
    let autoremove = "==> Would autoremove 2 unneeded formulae:\nlibfoo\nbar\n";
    assert_eq!(parse_brew_autoremove(autoremove), vec!["libfoo", "bar"]);
    assert!(parse_brew_autoremove("").is_empty());
}

#[derive(Default)]
struct FakeBrew {
    ran: Mutex<Vec<ToolCommand>>,
}
impl ToolProvider for FakeBrew {
    fn brew_available(&self) -> bool {
        true
    }
    fn brew_cleanup_preview(&self) -> Result<(u64, Vec<String>), String> {
        Ok((4096, vec!["Would remove: old".into()]))
    }
    fn brew_autoremove_preview(&self) -> Result<Vec<String>, String> {
        Ok(vec!["libfoo".into()])
    }
    fn run(&self, command: ToolCommand) -> Result<(), String> {
        self.ran.lock().unwrap().push(command);
        Ok(())
    }
}

#[test]
fn tool_items_are_never_preselected_and_always_permanent() {
    let temp = tempfile::tempdir().unwrap();
    let platform = TestPlatform::new(temp.path(), "");
    let mut brew = rule("brew-cleanup", Category::A, RuleKind::BrewCleanup);
    brew.regenerable = true;
    brew.preselect = true; // even a misconfigured catalog cannot preselect it
    let remove = rule("brew-autoremove", Category::B, RuleKind::BrewAutoremove);
    let tools = Arc::new(FakeBrew::default());
    let session = scan(
        &platform,
        &[brew, remove],
        &Settings::default(),
        tools.as_ref(),
    );
    assert_eq!(session.items.len(), 2);
    assert!(
        session
            .items
            .iter()
            .all(|i| !i.preselected && i.permanent_only)
    );
    assert_eq!(session.items[0].size_bytes, 4096);
    assert_eq!(session.items[1].details, vec!["libfoo"]);

    let ids: Vec<_> = session.items.iter().map(|i| i.id.clone()).collect();
    let (mut cleaner, _) = cleaner(platform, tools.clone());
    let preview = cleaner.preview(&session, &ids, false);
    assert!(preview.requires_irreversible_ack);
    assert!(preview.items.iter().all(|i| i.action == Action::Permanent));
    assert!(cleaner.run(&preview.id, false).is_err());
    assert!(tools.ran.lock().unwrap().is_empty());
    let run = cleaner.run(&preview.id, true).unwrap();
    assert!(run.results.iter().all(|r| r.outcome == Outcome::Succeeded));
    assert_eq!(
        *tools.ran.lock().unwrap(),
        vec![ToolCommand::BrewCleanup, ToolCommand::BrewAutoremove]
    );
}

struct FakeDocker;
impl ContainerProvider for FakeDocker {
    fn available(&self) -> bool {
        true
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
    fn dangling_images(&self) -> Result<Vec<ContainerImage>, String> {
        Ok(vec![ContainerImage {
            id: "d1".into(),
            repo_tag: "<none>:<none>".into(),
            size_bytes: 9,
            used: false,
            locally_built: true,
        }])
    }
    fn stopped_containers(&self) -> Result<Vec<StoppedContainer>, String> {
        Ok(vec![StoppedContainer {
            id: "c1".into(),
            name: "old".into(),
            size_bytes: 3,
        }])
    }
    fn unused_volumes(&self) -> Result<Vec<String>, String> {
        Ok(vec!["vol1".into()])
    }
}

#[test]
fn docker_dangling_stopped_and_volume_items_have_safe_defaults() {
    let temp = tempfile::tempdir().unwrap();
    let platform = TestPlatform::new(temp.path(), "");
    let mut volumes = rule("container-volumes", Category::C, RuleKind::ContainerVolumes);
    volumes.review_only = true;
    volumes.guidance = Some("Remove with docker volume rm".into());
    let rules = [
        rule(
            "container-dangling",
            Category::B,
            RuleKind::ContainerDanglingImages,
        ),
        rule("container-stopped", Category::C, RuleKind::ContainerStopped),
        volumes,
    ];
    let session = scan_with_tools(
        "s".into(),
        &platform,
        &rules,
        &Settings::default(),
        &FakeDocker,
        &NoTools,
        &AtomicBool::new(false),
        &NoopProgress,
    );
    assert_eq!(session.items.len(), 3);
    assert!(
        session
            .items
            .iter()
            .all(|i| !i.preselected && i.permanent_only)
    );
    assert!(matches!(
        session.items[0].target,
        Target::ContainerImage { .. }
    ));
    assert!(matches!(session.items[1].target, Target::Container { .. }));
    assert!(!session.items[2].selectable);
}

// ---------- US3: duplicates and idle files ----------

#[test]
fn duplicates_are_grouped_by_content_and_one_copy_is_always_kept() {
    let temp = tempfile::tempdir().unwrap();
    let platform = TestPlatform::new(temp.path(), "");
    let root = platform.home().join("Pictures");
    write(&root.join("a/photo.jpg"), b"same-bytes-0001");
    write(&root.join("b/photo copy.jpg"), b"same-bytes-0001");
    write(&root.join("c/photo 2.jpg"), b"same-bytes-0001");
    // Same size, different content: not a duplicate.
    write(&root.join("d/other.jpg"), b"diff-bytes-0001");
    write(&root.join("e/key.pem"), b"same-bytes-0001");
    let settings = Settings {
        review_roots: vec![root.clone()],
        duplicate_min_bytes: 1,
        ..Settings::default()
    };
    let session = scan(
        &platform,
        &[rule(
            "duplicate-files",
            Category::C,
            RuleKind::DuplicateFiles,
        )],
        &settings,
        &NoTools,
    );

    assert_eq!(session.items.len(), 3, "{:?}", session.items);
    let group = session.items[0].duplicate_group.clone().unwrap();
    assert!(
        session
            .items
            .iter()
            .all(|i| i.duplicate_group.as_deref() == Some(group.as_str()) && !i.preselected)
    );

    let ids: Vec<_> = session.items.iter().map(|i| i.id.clone()).collect();
    let (mut cleaner, _) = cleaner(platform, Arc::new(NoTools));
    let preview = cleaner.preview(&session, &ids, false);
    assert_eq!(preview.items.len(), 2);
    assert_eq!(preview.refused.len(), 1);
    assert_eq!(preview.refused[0].reason, RefusalReason::KeepOneCopy);
    let partial = cleaner.preview(&session, &ids[..2], false);
    assert_eq!(partial.items.len(), 2);
}

#[test]
fn large_files_report_idle_days() {
    let temp = tempfile::tempdir().unwrap();
    let platform = TestPlatform::new(temp.path(), "");
    write(&platform.home().join("big.bin"), &[0; 64]);
    // Never walked: privacy-guarded app containers.
    write(
        &platform
            .library_dir()
            .unwrap()
            .join("Containers/com.example.x/Data/huge.bin"),
        &[0; 64],
    );
    let mut large = rule("large-files", Category::C, RuleKind::LargeFiles);
    large.paths = vec!["~".into()];
    let settings = Settings {
        large_file_threshold_bytes: 32,
        ..Settings::default()
    };
    let session = scan(&platform, &[large], &settings, &NoTools);
    assert_eq!(session.items.len(), 1);
    assert_eq!(session.items[0].idle_days, Some(0));
}

// ---------- US4: startup items ----------

#[derive(Default)]
struct FakeLaunch {
    disabled: Mutex<HashMap<String, bool>>,
    calls: Mutex<Vec<String>>,
}
impl LaunchControl for FakeLaunch {
    fn disabled_labels(&self, _uid: u32) -> HashMap<String, bool> {
        self.disabled.lock().unwrap().clone()
    }
    fn set_enabled(&self, uid: u32, label: &str, enabled: bool) -> Result<(), String> {
        self.calls.lock().unwrap().push(format!(
            "{} gui/{uid}/{label}",
            if enabled { "enable" } else { "disable" }
        ));
        self.disabled.lock().unwrap().insert(label.into(), !enabled);
        Ok(())
    }
}

#[test]
fn print_disabled_output_is_parsed() {
    let text = "disabled services = {\n\t\"com.example.a\" => disabled\n\t\"com.example.b\" => enabled\n\t\"com.example.c\" => true\n}\n";
    let map = startup::parse_print_disabled(text);
    assert_eq!(map.get("com.example.a"), Some(&true));
    assert_eq!(map.get("com.example.b"), Some(&false));
    assert_eq!(map.get("com.example.c"), Some(&true));
}

#[test]
fn startup_items_list_by_scope_and_only_user_agents_toggle() {
    let temp = tempfile::tempdir().unwrap();
    let platform = TestPlatform::new(temp.path(), "");
    let dirs = platform.launch_dirs();
    write(
        &dirs[0].0.join("com.example.agent.plist"),
        br#"{"Label":"com.example.agent","ProgramArguments":["/opt/example/agent","--flag"]}"#,
    );
    write(
        &dirs[1].0.join("com.example.daemon.plist"),
        br#"{"Label":"com.example.daemon","Program":"/opt/example/daemon","Disabled":true}"#,
    );
    let launch = FakeLaunch::default();
    let audit = AuditLog::new(temp.path().join("audit.jsonl"));

    let items = startup::list_items(&platform, &launch);
    assert_eq!(items.len(), 2);
    let agent = items
        .iter()
        .find(|i| i.label == "com.example.agent")
        .unwrap();
    assert_eq!(agent.scope, StartupScope::UserAgent);
    assert!(agent.enabled && agent.editable);
    assert_eq!(agent.program.as_deref(), Some("/opt/example/agent"));
    let daemon = items
        .iter()
        .find(|i| i.label == "com.example.daemon")
        .unwrap();
    assert!(!daemon.enabled && !daemon.editable);

    assert!(matches!(
        startup::set_enabled(
            &platform,
            &launch,
            &audit,
            "com.example.agent",
            false,
            false
        ),
        Err(StartupError::AckRequired)
    ));
    let updated =
        startup::set_enabled(&platform, &launch, &audit, "com.example.agent", false, true).unwrap();
    assert!(!updated.enabled);
    assert_eq!(
        *launch.calls.lock().unwrap(),
        vec!["disable gui/501/com.example.agent"]
    );
    assert!(
        std::fs::read_to_string(audit.path())
            .unwrap()
            .contains("startupToggle")
    );

    assert!(matches!(
        startup::set_enabled(
            &platform,
            &launch,
            &audit,
            "com.example.daemon",
            false,
            true
        ),
        Err(StartupError::NotEditable)
    ));
    assert!(matches!(
        startup::set_enabled(&platform, &launch, &audit, "bad label;rm", false, true),
        Err(StartupError::InvalidLabel)
    ));
    assert_eq!(launch.calls.lock().unwrap().len(), 1);
    // The audit log with a startup event still reads as cleanup history.
    assert!(audit.read_runs().unwrap().is_empty());
}
