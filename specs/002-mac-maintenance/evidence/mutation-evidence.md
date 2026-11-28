# Regression evidence: targeted mutations (spec 002)

Tester FAIL on PR #3 @06b8245 asked for runnable baseline evidence for net-new
maintenance behaviour. The base branch has no apps/startup/tools APIs, so a copied
test cannot compile there (non-probative). Instead `scripts/mutation-evidence.py`
removes, at the exact head, the single behaviour each test guards, runs only that
test with `--exact`, and requires a FAILED test whose panic is an `assert!`/`assert_eq!` failure
raised inside that test's own source file. Panics from unwrap/expect, indexing or
`panic!`, panics in other files, and compile errors all count as failed evidence. Each test must also pass unmutated first.

Reproduce: `python3 -I scripts/mutation-evidence.py` (exit 0 = all killed).

| Test | Behaviour removed by the mutation |
|---|---|
| app_scan_lists_bundles_with_exact_leftovers_and_marks_apple_apps | Apple apps detected and not removable |
| uninstall_moves_bundle_and_leftovers_to_trash_even_when_permanent_requested | uninstall forced to Trash |
| running_or_apple_apps_are_refused_and_untouched | running apps refused at preview |
| app_started_after_preview_is_skipped_as_in_use | running state re-checked at uninstall |
| guard_accepts_app_bundles_only_through_the_uninstaller_exception | `.app` exception needs a real bundle directory |
| brew_dry_run_output_is_parsed | brew cleanup dry-run lines recognised |
| tool_items_are_never_preselected_and_always_permanent | tool items unselected, permanent-only |
| docker_dangling_stopped_and_volume_items_have_safe_defaults | container items unselected, permanent-only |
| duplicates_are_grouped_by_content_and_one_copy_is_always_kept | one copy per duplicate group kept |
| large_files_report_idle_days | idle days reported |
| print_disabled_output_is_parsed | launchctl print-disabled parsing |
| startup_items_list_by_scope_and_only_user_agents_toggle | only user LaunchAgents toggleable |
| scanner::walk_tests::walk_never_enters_protected_or_claimed_subtrees | walk prunes guard-protected subtrees |
| large_files_report_idle_days (privacy) | walk never opens privacy-guarded app containers |

The privacy exclusion also has a base black-box probe (Tester, PR #3 FAIL comment):
base walks `Library/Containers`, the head test passes.

## Run output

```
head 7efbb7402dc0
KILLED app_scan_lists_bundles_with_exact_leftovers_and_marks_apple_apps
       removed: Apple apps are detected and marked not removable
       thread 'app_scan_lists_bundles_with_exact_leftovers_and_marks_apple_apps' (10851470) panicked at crates/ttops-core/tests/maintenance.rs:164:5:
       assertion failed: !safari.selectable
KILLED uninstall_moves_bundle_and_leftovers_to_trash_even_when_permanent_requested
       removed: uninstall action is forced to Trash
       thread 'uninstall_moves_bundle_and_leftovers_to_trash_even_when_permanent_requested' (10852355) panicked at crates/ttops-core/tests/maintenance.rs:220:5:
       assertion `left == right` failed
KILLED running_or_apple_apps_are_refused_and_untouched
       removed: running apps are detected and refused
       thread 'running_or_apple_apps_are_refused_and_untouched' (10853222) panicked at crates/ttops-core/tests/maintenance.rs:246:5:
       assertion failed: preview.items.is_empty()
KILLED app_started_after_preview_is_skipped_as_in_use
       removed: running state is re-checked at uninstall time
       thread 'app_started_after_preview_is_skipped_as_in_use' (10854064) panicked at crates/ttops-core/tests/maintenance.rs:319:5:
       assertion `left == right` failed
KILLED guard_accepts_app_bundles_only_through_the_uninstaller_exception
       removed: the .app exception requires a real bundle directory
       thread 'guard_accepts_app_bundles_only_through_the_uninstaller_exception' (10854914) panicked at crates/ttops-core/tests/maintenance.rs:345:9:
       assertion failed: guard.check_app_bundle(&link, &app_dirs).is_err()
KILLED brew_dry_run_output_is_parsed
       removed: brew cleanup dry-run lines are recognised
       thread 'brew_dry_run_output_is_parsed' (10855752) panicked at crates/ttops-core/tests/maintenance.rs:360:5:
       assertion `left == right` failed
KILLED tool_items_are_never_preselected_and_always_permanent
       removed: external tool items are unselected and permanent-only
       thread 'tool_items_are_never_preselected_and_always_permanent' (10856616) panicked at crates/ttops-core/tests/maintenance.rs:402:5:
       assertion failed: session.items.iter().all(|i| !i.preselected && i.permanent_only)
KILLED docker_dangling_stopped_and_volume_items_have_safe_defaults
       removed: container items are unselected and permanent-only
       thread 'docker_dangling_stopped_and_volume_items_have_safe_defaults' (10857512) panicked at crates/ttops-core/tests/maintenance.rs:491:5:
       assertion failed: session.items.iter().all(|i| !i.preselected && i.permanent_only)
KILLED duplicates_are_grouped_by_content_and_one_copy_is_always_kept
       removed: one copy of each duplicate group is always kept
       thread 'duplicates_are_grouped_by_content_and_one_copy_is_always_kept' (10858344) panicked at crates/ttops-core/tests/maintenance.rs:546:5:
       assertion `left == right` failed
KILLED large_files_report_idle_days
       removed: large files carry idle days
       thread 'large_files_report_idle_days' (10859226) panicked at crates/ttops-core/tests/maintenance.rs:574:5:
       assertion `left == right` failed
KILLED print_disabled_output_is_parsed
       removed: launchctl print-disabled states are parsed
       thread 'print_disabled_output_is_parsed' (10860119) panicked at crates/ttops-core/tests/maintenance.rs:602:5:
       assertion `left == right` failed
KILLED startup_items_list_by_scope_and_only_user_agents_toggle
       removed: only user LaunchAgents are toggleable
       thread 'startup_items_list_by_scope_and_only_user_agents_toggle' (10861006) panicked at crates/ttops-core/tests/maintenance.rs:636:5:
       assertion failed: !daemon.enabled && !daemon.editable
KILLED scanner::walk_tests::walk_never_enters_protected_or_claimed_subtrees
       removed: large-file walk prunes guard-protected subtrees
       thread 'scanner::walk_tests::walk_never_enters_protected_or_claimed_subtrees' (10861461) panicked at crates/ttops-core/src/scanner.rs:609:13:
       assertion `left == right` failed: walk entered "/var/folders/xv/r4_nd_qn35z67pcvr_6y7b3c0000gn/T/.tmpw5QUAL/home/.ssh"
KILLED large_files_report_idle_days
       removed: large-file walk never opens privacy-guarded app containers
       thread 'large_files_report_idle_days' (10862322) panicked at crates/ttops-core/tests/maintenance.rs:573:5:
       assertion `left == right` failed

14/14 mutations killed
```
