# Implementation Plan: Mac Maintenance Toolkit

**Branch**: `002-mac-maintenance` | **Spec**: [spec.md](spec.md)

## Summary

Extend the existing scan → preview → cleaner pipeline instead of building parallel engines. New
scan sources produce `ScanItem`s with new `Target` variants; the cleaner executes them with the same
guard, Trash default, audit, and per-item results. Startup items are a separate feature module.

## Technical Context

Rust core (`ttops-core`), Tauri 2 shell, TypeScript UI. No new crates: Info.plist and launchd plists
are read with the built-in `plutil`, launch state with `launchctl`, tools via their CLIs. Content
hashing uses the standard library hasher plus byte-for-byte confirmation.

## Constitution Check (1.2.0)

- I Safety-First: Trash default; uninstall Trash-only under the 1.2.0 exception; tool cleanups
  permanent with ack; startup toggles user-scope only, audited. PASS.
- II Parity: macOS-first; documented gap FR-009. PASS (documented exception).
- III Modular: new core modules `apps`, `duplicates`, `startup`, `tools`; UI views Apps and Startup
  registered in the feature registry. PASS.
- IV Test-first: tests written per module on temp dirs with fake providers; guard regression for
  `.app` refusal outside the uninstaller. PASS.
- V Lightweight/public-safe: no new dependencies, no hard-coded personal paths. PASS.

## Design

- `platform`: `app_dirs()`, `library_dir()`, `launch_dirs()`, `bundle_info()`, `user_id()`;
  `TestPlatform` reads `Contents/test-info.json` instead of `plutil`.
- `guard`: `check_app_bundle(path, app_dirs)` — the single `.app` exception.
- `apps`: scan apps + leftovers + orphans → `Target::App { path, bundleId, executable, leftovers }`.
- `tools`: `ToolProvider` trait + `BrewProvider` (cleanup/autoremove dry-run parsing);
  `Target::Tool { command }`. `ContainerProvider` gains dangling images, stopped containers, unused
  volumes (default no-op methods keep existing fakes valid).
- `duplicates`: size bucket → hash → byte compare; `ScanItem.duplicateGroup`; cleaner preview
  refusal `keepOneCopy`.
- Large files gain `idleDays`.
- `startup`: list items; `LaunchControl` trait (`launchctl`) for state and toggle; audit event.
- Tauri: `start_app_scan`, `list_startup_items`, `set_startup_item_enabled`; settings gain
  `reviewRoots`. `ScanStore` keeps the latest session per kind.
