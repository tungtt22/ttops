# Regression evidence: protected-subtree walk (PR #2 F1)

Tester FAIL on PR #2 @50e9ec3: the F1 regression is an inline unit test of
`walk_unprotected`, which does not exist at predecessor ac9ab3d, so the same
selector runs 0 tests there.

Predecessor ac9ab3d skipped only claimed paths in the large-file walk
(`scanner.rs` loop: `if claimed.iter().any(|claim| path.starts_with(claim))`).
The head predicate is that clause plus `guard.protects_subtree(path)` for
directories below the root. `scripts/mutation-evidence.py` removes exactly the
added clause — restoring the predecessor predicate — runs only the regression,
and requires a FAILED test whose panic is an `assert!`/`assert_eq!` failure inside
`crates/ttops-core/src/scanner.rs`. Any other panic or a compile error counts as
failed evidence.

Reproduce: `python3 -I scripts/mutation-evidence.py` (exit 0 = killed).

## Run output

```
head ea825f3335cb
KILLED scanner::walk_tests::walk_never_enters_protected_or_claimed_subtrees
       removed: large-file walk prunes guard-protected subtrees
       thread 'scanner::walk_tests::walk_never_enters_protected_or_claimed_subtrees' (10844821) panicked at crates/ttops-core/src/scanner.rs:479:13:
       assertion `left == right` failed: walk entered "/var/folders/xv/r4_nd_qn35z67pcvr_6y7b3c0000gn/T/.tmpYwp7ku/home/.ssh"

1/1 mutations killed
```
