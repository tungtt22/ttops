# Implementation Plan: Storage Scan and Clean

**Branch**: `001-storage-scan-clean` | **Date**: 2026-11-22 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/001-storage-scan-clean/spec.md`

## Summary

Build the first TTops feature and the app shell: a cross-platform desktop app that shows system
volume usage, runs a read-only, cancellable scan producing categorised reclaimable items, and
performs guarded cleanup (preview → confirm → Trash by default) with a JSONL audit log shown in a
History view. All filesystem and container logic lives in a Tauri-independent Rust library
(`ttops-core`) so it is tested on temporary directories; the Tauri app crate is a thin command
layer; the UI is vanilla TypeScript with a feature registry driving the sidebar.

## Technical Context

**Language/Version**: Rust 1.97 (edition 2024); TypeScript 5 on Node 24

**Primary Dependencies**: Tauri 2.12, `trash` 5, `sysinfo` 0.40, `walkdir` 2, `rayon` 1,
`serde`/`serde_json`, `toml`, `thiserror`, `dirs`; dev: `tempfile`. UI: Vite, `@tauri-apps/api` 2

**Storage**: Local files only — `audit.jsonl` and `settings.json` in the OS app-data/config dirs

**Testing**: `cargo test` (unit + integration on `tempfile` trees, fake platform and fake container
provider); `tsc --noEmit` for the UI

**Target Platform**: macOS 13+ (arm64, x64), Windows 10/11 x64

**Project Type**: desktop-app (Rust core library + Tauri shell + web UI)

**Performance Goals**: ~300 GB home scanned in < 3 min (SC-002); UI remains interactive during scans
(work on background threads, batched progress events ≤ 10/s)

**Constraints**: offline, no telemetry; release binary < 20 MB; non-admin user only; no symlink
following during size calculation or deletion

**Scale/Scope**: single user; ~40 cleanup rules per OS; up to millions of files per scan

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | Status | How the design complies |
|-----------|--------|-------------------------|
| I. Safety-First Cleanup | PASS | Scanner has no write API; cleanup requires a server-side preview id and, for permanent mode, an irreversible acknowledgement; `Guard` denylist checked on canonicalised paths; Trash default; only `regenerable && preselect` rules preselected; JSONL audit per run. |
| II. Cross-Platform Parity | PASS | `Platform` trait with `macos`/`windows` impls; rule catalogs are per-OS TOML data; UI has no OS branches (it renders what the core returns). |
| III. Modular Features | PASS | Core modules `storage` (scan/clean) and `audit`; shared `fsutil`, `guard`, `platform`; UI features registered in `ui/src/features/registry.ts`. |
| IV. Test-First Core | PASS | Guard, rule expansion, scanner, cleaner tests written first against `tempfile` dirs via an injectable `Platform` whose home/trash point into temp dirs; no test touches real user dirs. |
| V. Lightweight & Private | PASS | No network crates; container ops via local CLI; vanilla TS UI. |
| Workflow gates | PASS | fmt, clippy `-D warnings`, test, `tsc` scripted in `scripts/check.sh`; PR-only flow documented in `CONTRIBUTING.md`. |

Post-design re-check: PASS (no exceptions; Complexity Tracking empty).

## Project Structure

### Documentation (this feature)

```text
specs/001-storage-scan-clean/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   └── tauri-commands.md
└── tasks.md            # created by $speckit-tasks
```

### Source Code (repository root)

```text
Cargo.toml                    # workspace: crates/ttops-core, src-tauri
crates/ttops-core/
├── Cargo.toml
├── rules/
│   ├── macos.toml
│   └── windows.toml
├── src/
│   ├── lib.rs
│   ├── model.rs              # Category, Rule, ScanItem, Preview, CleanupRun, SkipReason
│   ├── platform/{mod.rs, macos.rs, windows.rs}
│   ├── rules.rs              # catalog parsing + path token expansion
│   ├── guard.rs              # protected-path checks
│   ├── fsutil.rs             # size calc, last-modified, no symlink follow
│   ├── container.rs          # ContainerProvider trait + docker CLI impl
│   ├── scanner.rs            # parallel scan, cancel, progress sink
│   ├── cleaner.rs            # preview store, execute with re-check
│   ├── audit.rs              # JSONL append + read + interrupted detection
│   └── settings.rs
└── tests/                    # integration tests on tempfile trees
src-tauri/
├── Cargo.toml, tauri.conf.json, build.rs, capabilities/default.json, icons/
└── src/{main.rs, commands.rs, state.rs}
ui/
├── index.html, package.json, tsconfig.json, vite.config.ts
└── src/
    ├── main.ts, api.ts, styles.css
    └── features/{registry.ts, storage/, history/}
scripts/check.sh
```

**Structure Decision**: Cargo workspace with a pure library crate (`ttops-core`) holding every rule
and side effect behind traits, so Principle IV tests never need Tauri or real user state; the Tauri
crate only maps commands/events; the UI lives in `ui/` and is built by Vite into `src-tauri`'s
`frontendDist`.

## Complexity Tracking

No constitution violations.
