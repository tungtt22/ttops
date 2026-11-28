# Tasks: Mac Maintenance Toolkit

## Phase 1: Foundation
- [x] T001 Constitution 1.2.0 amendment (uninstaller exception, tool cleanups, startup toggles)
- [x] T002 Platform: app dirs, library dir, launch dirs, bundle info, user id; TestPlatform support
- [x] T003 Model: `Target::App`, `Target::Tool`, `Target::Container`, `duplicateGroup`, `idleDays`, `RefusalReason::KeepOneCopy`, settings `reviewRoots`

## Phase 2: US1 App uninstaller (P1)
- [x] T004 Tests: app listing, Apple/running/symlink refusal, leftovers, orphans, Trash-only run, guard still refuses `.app` elsewhere
- [x] T005 Guard `check_app_bundle`; `apps` scan module; cleaner `Target::App` execution
- [x] T006 Tauri `start_app_scan`; UI Apps view

## Phase 3: US2 Extended junk and toolchains (P1)
- [x] T007 Tests: brew dry-run parsing, docker list parsing, tool items permanent and not preselected
- [x] T008 `tools` module (Homebrew), container extensions, scanner rule kinds, macOS catalog rules
- [x] T009 Cleaner `Target::Tool` / `Target::Container` execution with audit

## Phase 4: US3 Duplicates and idle files (P2)
- [x] T010 Tests: grouping, byte confirmation, protected exclusion, keep-one-copy refusal, idle days
- [x] T011 `duplicates` module, scanner integration, preview refusal, UI review folders and idle filter

## Phase 5: US4 Startup items (P2)
- [x] T012 Tests: listing by scope, enabled state parsing, user-only toggle, label validation, audit
- [x] T013 `startup` module, Tauri commands, UI Startup view

## Phase 6: Polish
- [x] T014 `scripts/check.sh`, app build, docs and handoff

## Implementation notes and deviations (2026-11-27)

- Tests: `crates/ttops-core/tests/maintenance.rs` (12) plus catalog validation in `tests/rules.rs`.
- Privacy-guarded folders: macOS blocks on a consent prompt when a process opens another app's
  `~/Library/Containers` or `~/Library/Group Containers`. Found in a read-only smoke run (the scan
  hung). TTops now lists these leftovers but never reads inside them to size them (size shown as 0
  with a note), and the large-file and duplicate walks skip them. The same skip also fixes the
  large-file walk from `001-storage-bc`.
- Startup: `launchctl disable` takes effect at next login; running agents are not stopped (keeps
  the change reversible). Login items registered through Background Task Management are not listed
  (need elevated rights); the view points to System Settings.
- Catalog validation now rejects external tool rules that are preselected or not `permanentOnly`.
- Release smoke (read-only, owner machine): app scan 45 apps / 26 orphan candidates in ~21 s (debug
  build), startup 18 items in 67 ms, `brew cleanup --dry-run` 1.2 GiB in 1.7 s.
- Not done: interactive UI walkthrough, uninstall on a real app, Windows.

## Carried from PR #1 review remediation

- [x] Merged the PR #1 fix (6a34ba8): strict Tauri CSP, vitest UI tests in `check.sh`.
- [x] `ui/src/features/storage/clean.ts` (rewritten in this slice) now builds the preview — labels, detail paths, refusal reasons — with DOM nodes and `textContent` instead of escaped `innerHTML`; `clean.test.ts` adapted to the backend-driven preview.
- [x] Merged PR #2 remediation (50e9ec3): `walk_unprotected` now takes the platform and skips both guard-protected subtrees and privacy-guarded app containers; `duplicates.rs` walk also skips guard-protected subtrees; storage view recomputes `Clean selected` on every selection change (`view.test.ts` adapted for `details`).
- [x] Tester FAIL @06b8245 finding 2: added `scripts/mutation-evidence.py` and `evidence/mutation-evidence.md` (14/14 targeted mutations killed by assertion panics).
- [x] Tech Lead REQUEST_CHANGES @f20c0e6: evidence now requires an assertion panic in the test's own file (unwrap/other panics rejected); trailing whitespace removed.
