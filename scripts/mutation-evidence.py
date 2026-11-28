#!/usr/bin/env python3
"""Regression evidence by targeted mutation.

Some regressions exercise code that does not exist on the predecessor, so the
same test selects nothing or cannot compile there. Instead, for each test this script removes the one
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
    # Predecessor ac9ab3d skipped only claimed paths; removing the protects_subtree
    # clause restores exactly that walk predicate.
    ("lib", "scanner::walk_tests::walk_never_enters_protected_or_claimed_subtrees",
     CORE + "scanner.rs", "\n            || (entry.depth() > 0 && entry.file_type().is_dir() && guard.protects_subtree(path));", ";",
     "large-file walk prunes guard-protected subtrees", "crates/ttops-core/src/scanner.rs"),
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
