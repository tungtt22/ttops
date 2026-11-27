use ttops_core::{
    audit::AuditLog,
    model::{Action, CleanupRun, PreviewItem, RunStatus, Target},
};

fn run(id: &str, started_at: u64) -> CleanupRun {
    CleanupRun {
        id: id.into(),
        preview_id: format!("preview-{id}"),
        started_at,
        finished_at: None,
        mode: Action::Trash,
        status: RunStatus::Running,
        items: vec![PreviewItem {
            item_id: "item".into(),
            rule_id: "rule".into(),
            label: "item".into(),
            target: Target::ContainerBuildCache,
            size_bytes: 12,
            action: Action::Trash,
            details: vec![],
        }],
        results: vec![],
        freed_bytes: 0,
    }
}

#[test]
fn reads_completed_runs_newest_first_and_marks_unfinished_runs_interrupted() {
    let temp = tempfile::tempdir().unwrap();
    let audit = AuditLog::new(temp.path().join("audit.jsonl"));
    let old = run("old", 10);
    let unfinished = run("unfinished", 30);
    let newest = run("newest", 40);
    audit.run_started(&old).unwrap();
    audit.run_finished(&old.id, 12).unwrap();
    audit.run_started(&unfinished).unwrap();
    audit.run_started(&newest).unwrap();
    audit.run_finished(&newest.id, 12).unwrap();

    let runs = audit.read_runs().unwrap();
    assert_eq!(
        runs.iter().map(|run| run.id.as_str()).collect::<Vec<_>>(),
        ["newest", "unfinished", "old"]
    );
    assert_eq!(runs[0].status, RunStatus::Completed);
    assert_eq!(runs[1].status, RunStatus::Interrupted);
    assert_eq!(runs[2].freed_bytes, 12);
}
