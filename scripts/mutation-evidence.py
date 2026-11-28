#!/usr/bin/env python3
"""Regression evidence for net-new maintenance behaviour (spec 002).

The 002 tests exercise APIs that do not exist on the base branch, so a copied
test cannot compile there. Instead, for each test this script removes the one
behaviour the test guards (a targeted mutation of the exact head), runs only
that test, and requires it to fail at runtime with an assertion panic, never a
compile error. The unmutated head must pass the same tests.

Usage: python3 -I scripts/mutation-evidence.py   (from the repository root)
Exit code 0 means every mutation was killed for the intended reason.
"""
import os
import subprocess
import sys
import tempfile
from pathlib import Path

CORE = "crates/ttops-core/src/"

# (test target, test name, file, original text, mutated text, behaviour removed,
#  source file whose test body must raise the assertion panic)
MUTATIONS = [
    ("maintenance", "app_scan_lists_bundles_with_exact_leftovers_and_marks_apple_apps",
     CORE + "apps.rs", 'bundle_id.to_ascii_lowercase().starts_with("com.apple.")', "false",
     "Apple apps are detected and marked not removable", "crates/ttops-core/tests/maintenance.rs"),
    ("maintenance", "uninstall_moves_bundle_and_leftovers_to_trash_even_when_permanent_requested",
     CORE + "cleaner.rs", "Target::App { .. } => Action::Trash,",
     "Target::App { .. } if permanent => Action::Permanent,\n                Target::App { .. } => Action::Trash,",
     "uninstall action is forced to Trash", "crates/ttops-core/tests/maintenance.rs"),
    ("maintenance", "running_or_apple_apps_are_refused_and_untouched",
     CORE + "apps.rs", "!executable.is_empty() && running.contains(&executable)", "false",
     "running apps are detected and refused", "crates/ttops-core/tests/maintenance.rs"),
    ("maintenance", "app_started_after_preview_is_skipped_as_in_use",
     CORE + "cleaner.rs", "if crate::apps::is_running(executable, &self.platform.running_processes()) {",
     "if false {", "running state is re-checked at uninstall time", "crates/ttops-core/tests/maintenance.rs"),
    ("maintenance", "guard_accepts_app_bundles_only_through_the_uninstaller_exception",
     CORE + "guard.rs", "if !metadata.is_dir()\n            || !path", "if !path",
     "the .app exception requires a real bundle directory", "crates/ttops-core/tests/maintenance.rs"),
    ("maintenance", "brew_dry_run_output_is_parsed",
     CORE + "tools.rs", 'line.starts_with("Would remove:")', 'line.starts_with("Would delete:")',
     "brew cleanup dry-run lines are recognised", "crates/ttops-core/tests/maintenance.rs"),
    ("maintenance", "tool_items_are_never_preselected_and_always_permanent",
     CORE + "scanner.rs", "    found.preselected = false;\n    found.permanent_only = true;\n", "",
     "external tool items are unselected and permanent-only", "crates/ttops-core/tests/maintenance.rs"),
    ("maintenance", "docker_dangling_stopped_and_volume_items_have_safe_defaults",
     CORE + "scanner.rs", "    found.preselected = false;\n    found.permanent_only = true;\n", "",
     "container items are unselected and permanent-only", "crates/ttops-core/tests/maintenance.rs"),
    ("maintenance", "duplicates_are_grouped_by_content_and_one_copy_is_always_kept",
     CORE + "cleaner.rs",
     "fn keep_one_copy(scan: &ScanSession, requested: &HashSet<&String>) -> HashSet<String> {",
     "fn keep_one_copy(scan: &ScanSession, requested: &HashSet<&String>) -> HashSet<String> {\n    if true { return HashSet::new(); }",
     "one copy of each duplicate group is always kept", "crates/ttops-core/tests/maintenance.rs"),
    ("maintenance", "large_files_report_idle_days",
     CORE + "scanner.rs", "found.idle_days = idle_days(&metadata);", "found.idle_days = None;",
     "large files carry idle days", "crates/ttops-core/tests/maintenance.rs"),
    ("maintenance", "print_disabled_output_is_parsed",
     CORE + "startup.rs", '"disabled" | "true" => true,', '"true" => true,',
     "launchctl print-disabled states are parsed", "crates/ttops-core/tests/maintenance.rs"),
    ("maintenance", "startup_items_list_by_scope_and_only_user_agents_toggle",
     CORE + "startup.rs", "editable: scope == StartupScope::UserAgent && valid_label(&label),",
     "editable: valid_label(&label),", "only user LaunchAgents are toggleable", "crates/ttops-core/tests/maintenance.rs"),
    ("lib", "scanner::walk_tests::walk_never_enters_protected_or_claimed_subtrees",
     CORE + "scanner.rs", "\n            || (entry.depth() > 0 && entry.file_type().is_dir() && guard.protects_subtree(path));", ";",
     "large-file walk prunes guard-protected subtrees", "crates/ttops-core/src/scanner.rs"),
    ("maintenance", "large_files_report_idle_days",
     CORE + "scanner.rs", "            || (entry.file_type().is_dir() && crate::apps::is_privacy_guarded(platform, path))\n", "",
     "large-file walk never opens privacy-guarded app containers", "crates/ttops-core/tests/maintenance.rs"),
]


def run_test(tree: Path, target: str, name: str, target_dir: Path) -> subprocess.CompletedProcess:
    env = {**os.environ, "CARGO_TARGET_DIR": str(target_dir)}
    selector = ["--lib"] if target == "lib" else ["--test", target]
    return subprocess.run(
        ["cargo", "test", "-q", "-p", "ttops-core", *selector, name, "--", "--exact"],
        cwd=tree, capture_output=True, text=True, env=env,
    )


def assertion_panic(output: str, test_file: str):
    """Returns (panic line, assertion line) only when the test reported FAILED and
    panicked inside `test_file` with an `assert!`/`assert_eq!` failure. Panics from
    unwrap/expect, indexing, overflow or explicit `panic!` do not qualify."""
    if "FAILED" not in output:
        return None
    lines = [line.strip() for line in output.splitlines()]
    for index, line in enumerate(lines[:-1]):
        if " panicked at " in line and f"panicked at {test_file}:" in line:
            message = lines[index + 1]
            if message.startswith("assertion failed:") or message.startswith("assertion `left"):
                return line, message
    return None


def main() -> int:
    repo = Path(subprocess.check_output(["git", "rev-parse", "--show-toplevel"], text=True).strip())
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip()
    with tempfile.TemporaryDirectory() as tmp:
        tree = Path(tmp) / "tree"
        subprocess.run(["git", "worktree", "add", "-q", "--detach", str(tree), head], cwd=repo, check=True)
        failures = 0
        try:
            print(f"head {head[:12]}")
            for target, name, file, old, new, behaviour, test_file in MUTATIONS:
                path = tree / file
                original = path.read_text()
                baseline = run_test(tree, target, name, repo / "target" / "mutation")
                if baseline.returncode != 0 or "1 passed" not in baseline.stdout:
                    print(f"FAIL  {name}: does not pass unmutated\n{baseline.stdout[-800:]}{baseline.stderr[-800:]}")
                    failures += 1
                    continue
                if original.count(old) != 1:
                    print(f"FAIL  {name}: mutation anchor not found exactly once in {file}")
                    failures += 1
                    continue
                path.write_text(original.replace(old, new))
                try:
                    mutated = run_test(tree, target, name, repo / "target" / "mutation")
                finally:
                    path.write_text(original)
                output = mutated.stdout + mutated.stderr
                if "error[E" in output or "could not compile" in output:
                    print(f"FAIL  {name}: mutation did not compile (non-probative)\n{output[-800:]}")
                    failures += 1
                elif assertion_panic(output, test_file):
                    panic, message = assertion_panic(output, test_file)
                    print(f"KILLED {name}\n       removed: {behaviour}\n       {panic}\n       {message}")
                elif mutated.returncode != 0:
                    print(f"FAIL  {name}: failed without an assertion panic in {test_file} (non-probative)\n{output[-800:]}")
                    failures += 1
                else:
                    print(f"FAIL  {name}: mutation survived ({behaviour})")
                    failures += 1
        finally:
            subprocess.run(["git", "worktree", "remove", "--force", str(tree)], cwd=repo, check=False)
        print(f"\n{len(MUTATIONS) - failures}/{len(MUTATIONS)} mutations killed")
        return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
