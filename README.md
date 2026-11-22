# TTops

Cross-platform (macOS + Windows) desktop toolkit to scan and clean storage and, later, monitor
apps and system resources. Built with Tauri 2 (Rust core + TypeScript UI).

Status: pre-alpha. Feature 001 (storage scan and clean) is specified and in implementation.

- Constitution: `.specify/memory/constitution.md`
- Current feature: `specs/001-storage-scan-clean/` (spec, plan, tasks)
- Contributing and workflow: `CONTRIBUTING.md`

## Safety model

Scans are read-only. Every cleanup is preview → explicit confirm → move to Trash/Recycle Bin by
default; permanent delete is a per-run opt-in. Protected paths (system dirs, app bundles,
credential stores, SSH keys, cloud credentials) are always refused. Every run is audited locally.
No telemetry.

## Develop

```bash
./scripts/check.sh            # fmt, clippy, tests, UI typecheck
npm --prefix ui run tauri dev # run the app (after Phase 1 setup tasks are complete)
```
