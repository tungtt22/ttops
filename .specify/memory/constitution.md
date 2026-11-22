<!--
Sync Impact Report
- Version change: (template) → 1.0.0
- Modified principles: placeholders → I. Safety-First Cleanup, II. Cross-Platform Parity,
  III. Modular Feature Architecture, IV. Test-First Core (NON-NEGOTIABLE),
  V. Lightweight and Private
- Added sections: Technology and Platform Constraints, Development Workflow and Quality Gates
- Removed sections: none
- Deferred TODOs: none
-->

# TTops Constitution

## Core Principles

### I. Safety-First Cleanup

TTops deletes user data, so every destructive path is designed to fail safe.

- Scanning MUST be read-only. A scan MUST NOT create, modify, move, or delete any scanned item.
- Every cleanup MUST follow preview → explicit user confirmation → execute. The preview MUST show
  the exact items, their total size, and the action that will be taken.
- The default action MUST be move-to-Trash (macOS) or Recycle Bin (Windows). Permanent deletion
  MUST be an explicit per-run opt-in, never a persisted default.
- The deletion engine MUST refuse paths on a protected-path denylist (OS system directories,
  application bundles, credential stores such as Keychains and Windows Credential Manager data,
  SSH/GPG keys, cloud CLI credential files) regardless of how the request was produced.
- User documents (Desktop, Documents, Pictures, Downloads, and equivalents) MUST NOT be
  selected for cleanup by default; they are only cleaned when the user selects individual items.
- Every executed cleanup MUST append an audit record (timestamp, items, sizes, action, outcome)
  to a local audit log.
- A cleanup rule MUST declare whether its target is regenerable (cache, build output) or user
  data; only regenerable targets may be pre-selected.

Rationale: the owner's own cleanup of a production machine showed that read-only scans, explicit
confirmation, and preserving locally built artefacts prevent irreversible loss.

### II. Cross-Platform Parity

- macOS and Windows are first-class targets. Every user-facing feature MUST ship for both, or
  its spec MUST state the platform gap and the reason.
- OS-specific behaviour MUST live behind a platform abstraction layer in the Rust core
  (`platform::macos`, `platform::windows`). The UI MUST NOT contain OS-specific logic.
- Cleanup targets MUST be data-driven rule catalogs per OS, not hard-coded branches in engine code.

### III. Modular Feature Architecture

- Each feature (for example Storage Scan, Cleanup, Monitor) MUST be a self-contained Rust module
  exposing typed Tauri commands, paired with one UI view, and registered in a feature registry.
- Features MUST NOT call each other's internals; shared needs go into a core library module
  (filesystem walking, size calculation, audit log, platform layer).
- Adding a feature MUST NOT require changing other features' code beyond registry entries.

### IV. Test-First Core (NON-NEGOTIABLE)

- Rule catalogs, the protected-path guard, and the deletion engine MUST have tests written before
  implementation (red → green → refactor).
- Tests MUST operate only on temporary directories created by the test. Tests MUST NOT read or
  write real user directories, the real Trash, or real system state.
- A change to the deletion engine or protected-path guard MUST include a regression test that
  proves a protected path is refused.

### V. Lightweight and Private

- TTops MUST NOT send telemetry or analytics. The only permitted network call is an update check
  explicitly triggered or enabled by the user.
- Release binaries SHOULD stay under 20 MB per platform; a new dependency that adds more than
  2 MB to a release build MUST be justified in its plan.
- Monitoring data MUST stay on the local machine.

## Technology and Platform Constraints

- Shell: Tauri 2. Core: Rust (stable toolchain). UI: TypeScript with Vite; no heavyweight UI
  framework unless a plan justifies it.
- Targets: macOS 13+ (Apple Silicon and Intel), Windows 10/11 x64.
- Release artefacts: `.app`/`.dmg` for macOS, `.msi` for Windows.
- Versioning: SemVer for the application; breaking changes to the audit log format or rule
  catalog schema require a MINOR bump with a migration note before 1.0, MAJOR after 1.0.
- Repository content MUST NOT include credentials, private session content, raw logs, caches,
  databases, or build outputs.

## Development Workflow and Quality Gates

- Features and observable-behaviour changes follow the Spec Kit lifecycle:
  `specs/<feature>/spec.md` → `plan.md` → `tasks.md` → implementation. Bug fixes restoring
  specified behaviour and pure refactors may skip `specs/` but must state root cause, acceptance
  criteria, and verification in the pull request.
- Changes reach `main` only through a pull request merged by the owner. No direct or force push
  to `main`; no `--no-verify`.
- Before a pull request is opened: `cargo fmt --check`, `cargo clippy -- -D warnings`,
  `cargo test`, and the UI type check MUST pass.
- Review follows the owner's separation of duties: Tech Lead exact-head review and Tester
  verification run as independent sessions tracked on the Kanban board, and verdicts are posted
  on the pull request. The authoring session never reviews its own change.
- Commit messages, pull requests, code, and docs MUST NOT contain AI attribution lines.

## Governance

- This constitution supersedes other project practices. Plans MUST include a Constitution Check
  that lists each principle and states compliance or a justified exception.
- Amendments are made by pull request that updates this file, bumps the version (MAJOR: principle
  removal or redefinition; MINOR: new principle or materially expanded guidance; PATCH: wording),
  and records the change in the Sync Impact Report.
- Reviewers MUST verify Principle I and IV compliance on every change touching cleanup code.

**Version**: 1.0.0 | **Ratified**: 2026-11-22 | **Last Amended**: 2026-11-22
