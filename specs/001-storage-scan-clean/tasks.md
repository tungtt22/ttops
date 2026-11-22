# Tasks: Storage Scan and Clean

**Input**: Design documents from `/specs/001-storage-scan-clean/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/tauri-commands.md, quickstart.md

**Tests**: REQUIRED for core logic by Constitution Principle IV (test-first, temp dirs only).
Write each test task before its implementation task and confirm it fails first.

## Handoff status (2026-11-22)

Scoping is complete. Implementation was started and then handed off; continue from here.

- Done: T001 (workspace; `src-tauri` is temporarily removed from `members` until T004 creates it —
  re-add it in T004), T002, T005, T006.
- Drafted, not yet compiling — finish and verify, do not mark done until tests pass:
  - T007 `crates/ttops-core/src/model.rs` (complete draft; adds `RuleKind::Children`,
    `Target::RecycleBin`, per-item `Action` — see deviations below).
  - T008/T009 `crates/ttops-core/src/platform/{mod,macos,windows}.rs`. Known compile errors:
    sysinfo 0.40 API differences in `running_processes_sysinfo` and `volume_for`; rule catalogs
    `rules/macos.toml` and `rules/windows.toml` do not exist yet (T015).
  - T010 `crates/ttops-core/tests/guard.rs` written (red phase); `src/guard.rs` is a stub that
    allows everything — T011 must implement it.
  - Other `src/*.rs` modules are empty placeholders.
- Design deviations from data-model.md to reconcile in docs during implementation:
  - Rule kind `glob` replaced by `children` (each child of a dir is an item); wildcards in the last
    path segment are handled by path expansion for any kind.
  - `permanentOnly` applies per item (`PreviewItem.action`) instead of forcing the whole run;
    `requiresIrreversibleAck` = mode is permanent OR any item is permanent-only.
  - Windows Recycle Bin is `Target::RecycleBin` (trash crate `os_limited`), macOS Trash is a path.
  - Documents/Desktop are not protected roots (constitution: not preselected, but individually
    deletable); credential stores and SSH/cloud keys are.
  - Timestamps are Unix milliseconds.

## Format: `[ID] [P?] [Story] Description`

## Phase 1: Setup

- [X] T001 Create Cargo workspace `Cargo.toml` with members `crates/ttops-core`, `src-tauri`, edition 2024, shared release profile (`lto = true`, `opt-level = "s"`, `strip = true`)
- [X] T002 Create `crates/ttops-core/Cargo.toml` with deps serde, serde_json, toml, thiserror, dirs, walkdir, rayon, trash, sysinfo; dev-dep tempfile
- [ ] T003 [P] Create UI scaffold `ui/package.json`, `ui/tsconfig.json`, `ui/vite.config.ts`, `ui/index.html` (Vite + TypeScript, `@tauri-apps/api` 2, `@tauri-apps/cli` 2, `@tauri-apps/plugin-dialog` 2)
- [ ] T004 Create Tauri app `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json` (identifier `com.tungtt22.ttops`, frontendDist `../ui/dist`), `src-tauri/build.rs`, `src-tauri/capabilities/default.json`, generated icons
- [X] T005 [P] Create `scripts/check.sh` running `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`, `npm --prefix ui run typecheck`
- [X] T006 [P] Create `.gitignore` (target/, ui/node_modules/, ui/dist/, .DS_Store, *.log), `README.md`, `CONTRIBUTING.md` linking the constitution and Spec Kit lifecycle

## Phase 2: Foundational (blocks all stories)

- [X] T007 Define model types in `crates/ttops-core/src/model.rs` per data-model.md (Category, RuleKind, Rule, ScanItem, Target, ScanSession, CleanupPreview, CleanupRun, ItemResult, SkipReason, Settings; camelCase serde)
- [X] T008 Define `Platform` trait in `crates/ttops-core/src/platform/mod.rs` (home, cache/data/local_data dirs, trash dir, protected roots, rule catalog source, `move_to_trash`, `is_process_running`) plus `TestPlatform` rooted in a temp dir
- [X] T009 [P] Implement `crates/ttops-core/src/platform/macos.rs` and `crates/ttops-core/src/platform/windows.rs` (cfg-gated) with protected roots from research.md R6
- [X] T010 Write failing tests `crates/ttops-core/tests/guard.rs`: protected roots, inside-root paths, symlink into protected root, home itself, volume root, `.app` bundle, name patterns `*.keychain*`, `id_rsa*`, `id_ed25519*`, `*.pem`, `credentials`, `.env`
- [X] T011 Implement `crates/ttops-core/src/guard.rs` (canonicalise, prefix + pattern checks) until T010 passes
- [X] T012 [P] Write failing tests `crates/ttops-core/tests/fsutil.rs`: dir size without following symlinks, last-modified, missing path
- [X] T013 Implement `crates/ttops-core/src/fsutil.rs` until T012 passes
- [X] T014 Write failing tests `crates/ttops-core/tests/rules.rs`: token expansion, catalog validation ("`preselect` requires `regenerable`", "must be false for category B and C", "`guidance` required when `reviewOnly`", unique ids), both embedded catalogs load
- [X] T015 Implement `crates/ttops-core/src/rules.rs` and catalogs `crates/ttops-core/rules/macos.toml`, `crates/ttops-core/rules/windows.toml` until T014 passes
- [X] T016 [P] Write failing tests `crates/ttops-core/tests/audit.rs`: append/read runs, newest first, run without `runFinished` reads as `interrupted`
- [X] T017 Implement `crates/ttops-core/src/audit.rs` (JSONL, append + flush per record) until T016 passes
- [X] T018 [P] Implement `crates/ttops-core/src/settings.rs` (load/save JSON, default threshold 1 GiB) with tests in same file
- [X] T019 Implement app shell: `src-tauri/src/main.rs`, `src-tauri/src/state.rs`; UI `ui/src/main.ts`, `ui/src/api.ts`, `ui/src/styles.css`, `ui/src/features/registry.ts` (sidebar driven by registry)

## Phase 3: User Story 1 — See disk usage and reclaimable space (P1) 🎯 MVP

**Goal**: volume summary + read-only cancellable scan of category A (and C model stores) grouped with sizes.
**Independent Test**: seeded temp home → items/sizes match; before/after snapshot unchanged; cancel → `cancelled`.

- [X] T020 [P] [US1] Write failing tests `crates/ttops-core/tests/scanner.rs`: categories and sizes for seeded `dirContents`/`dir`/`glob` rules, read-only snapshot equality (SC-004), cancellation, unreadable dir listed in `unscanned`, missing paths silently skipped
- [X] T021 [P] [US1] Write failing tests for `ContainerProvider` with a fake provider in `crates/ttops-core/tests/container.rs`: build cache item, images used by containers excluded, images with empty `RepoDigests` excluded, provider unavailable ⇒ `containerAvailable = false`
- [X] T022 [US1] Implement `crates/ttops-core/src/container.rs` (trait + docker CLI impl per research R5) until T021 passes
- [X] T023 [US1] Implement `crates/ttops-core/src/scanner.rs` (rayon per rule, AtomicBool cancel, progress sink trait) until T020 passes
- [X] T024 [US1] Implement volume summary in `crates/ttops-core/src/platform/mod.rs` using sysinfo (longest mount prefix of home)
- [X] T025 [US1] Add commands `get_volume_summary`, `start_scan`, `cancel_scan`, `get_scan` and events `scan://progress`, `scan://finished` in `src-tauri/src/commands.rs`
- [X] T026 [US1] Build Storage view `ui/src/features/storage/view.ts`: volume bar, Scan/Cancel, grouped categories with sizes, incomplete label, unscanned list

## Phase 4: User Story 2 — Clean selected items safely (P1) 🎯 MVP

**Goal**: preview → confirm → Trash (default) / permanent (ack) with per-item results and audit.
**Independent Test**: seeded temp home → selected A items land in TestPlatform trash; reasons for skipped items.

- [X] T027 [P] [US2] Write failing tests `crates/ttops-core/tests/cleaner.rs`: preview has no side effects; unknown/reused preview rejected; permanent without ack changes nothing; trash mode moves items; vanished item ⇒ `noLongerExists`; protected path ⇒ `protected`; `processNames` running ⇒ `inUse`; `permanentOnly` forces permanent; container image removal via fake provider; audit records written
- [X] T028 [US2] Implement `crates/ttops-core/src/cleaner.rs` (preview store, re-stat + re-guard, error mapping per research R10, audit writes) until T027 passes
- [X] T029 [US2] Add commands `preview_cleanup`, `run_cleanup` and events `cleanup://progress`, `cleanup://finished` in `src-tauri/src/commands.rs`
- [X] T030 [US2] Add selection (A preselected; "select all" excludes C), preview dialog, irreversible second confirmation, progress and result panel in `ui/src/features/storage/clean.ts`

## Phase 5: User Story 3 — Developer build outputs (P2)

**Goal**: find `node_modules`, `target`, `.venv`, `dist`, `.next` under project roots with age.
**Independent Test**: temp roots with seeded projects of different ages → outermost outputs with size and age, none preselected.

- [ ] T031 [P] [US3] Write failing tests in `crates/ttops-core/tests/scanner.rs` for `projectOutputs`: outermost-only, project age days, not preselected, protected roots rejected by `set_settings`
- [ ] T032 [US3] Implement `projectOutputs` in `crates/ttops-core/src/scanner.rs` until T031 passes
- [ ] T033 [US3] Add `get_settings`, `set_settings`, `pick_project_root` commands in `src-tauri/src/commands.rs` (dialog plugin)
- [ ] T034 [US3] Add project roots management and category B listing with age in `ui/src/features/storage/view.ts`

## Phase 6: User Story 4 — Review large personal data (P2)

**Goal**: model stores, large files over threshold, chat media (review-only with guidance).
**Independent Test**: seeded large file + model dir → in C, not preselected; chat media not selectable.

- [ ] T035 [P] [US4] Write failing tests in `crates/ttops-core/tests/scanner.rs` for `largeFiles` (threshold, excludes paths already claimed by other rules) and `reviewOnly` items (`selectable = false`, guidance present)
- [ ] T036 [US4] Implement `largeFiles` and review-only handling in `crates/ttops-core/src/scanner.rs` until T035 passes
- [ ] T037 [US4] Render category C with guidance and individual-only selection in `ui/src/features/storage/view.ts`

## Phase 7: User Story 5 — Cleanup history (P3)

**Goal**: list runs and run details from the audit log.
**Independent Test**: two runs → both listed newest first after restart; interrupted run shown.

- [ ] T038 [US5] Add `list_runs`, `get_run` commands in `src-tauri/src/commands.rs`
- [ ] T039 [US5] Build History view `ui/src/features/history/view.ts` and register it in `ui/src/features/registry.ts`

## Phase 8: User Story 6 — Extensible app shell (P3)

**Goal**: sidebar registry supports adding features without touching others.
**Independent Test**: a placeholder "Monitor (coming soon)" entry registers with one registry line.

- [ ] T040 [US6] Add a placeholder Monitor entry in `ui/src/features/registry.ts` and document adding a feature in `docs/architecture.md`

## Phase 9: Polish & Cross-Cutting

- [ ] T041 [P] Write `docs/architecture.md` (crates, platform layer, rule catalog schema, safety protocol) and `docs/rules.md` (how to add a cleanup rule)
- [ ] T042 Run `scripts/check.sh` and fix all findings
- [ ] T043 Run `npm --prefix ui run tauri build` on macOS and record binary size against the < 20 MB constraint in `docs/architecture.md`
- [ ] T044 Execute quickstart.md manual checks on macOS and record results in `specs/001-storage-scan-clean/quickstart.md` (Windows run deferred until a Windows machine is available)

## Dependencies & Execution Order

- Phase 1 → Phase 2 → stories. US1 → US2 (cleaner consumes scan items). US3, US4 depend on the US1
  scanner only and can run in parallel after US1. US5 depends on US2 (audit runs). US6 depends on T019.
- Within a story: tests first (fail) → core → commands → UI.

## Parallel Examples

- Phase 2: T009, T012, T016, T018 in parallel after T008.
- US1: T020 and T021 in parallel; then T022 and T023.
- After US1: US3 (T031–T034) and US4 (T035–T037) in parallel.

## Implementation Strategy

1. MVP = Phases 1–4 (US1 + US2): scan and clean category A safely — reproduces the owner's manual cleanup (SC-007).
2. Then US3, US4 (more reclaimable space), US5 (history), US6 (shell readiness for Monitor).
3. Polish: docs, checks, release build.
