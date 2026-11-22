# TTops

Desktop toolkit to scan and clean storage and, later, monitor apps and system resources.
Built with Tauri 2 (Rust core + TypeScript UI) for macOS and Windows.

Repository: <https://github.com/tungtt22/ttops>

## Status

Pre-alpha. Roadmap:

| Version | Scope |
|---------|-------|
| v0.1.0 | macOS storage scan and clean: safe caches, package-manager caches, Docker build cache and unused registry images, Trash |
| v0.2.0 | Developer build outputs, large-file review, cleanup history |
| later | Windows support, app/system monitor |

Current feature: `specs/001-storage-scan-clean/` (spec, plan, tasks).

## Safety model

- Scans are read-only.
- Every cleanup is preview → explicit confirm → move to Trash by default; permanent delete is a
  per-run opt-in with a second confirmation.
- Protected paths (system directories, app bundles, credential stores, SSH keys, cloud
  credentials) are always refused, including through symlinks.
- Docker cleanup never stops containers and never removes images that are in use or built locally.
- Every cleanup run is recorded in a local audit log. No telemetry, no network calls.

## Develop

Prerequisites: Rust stable, Node 24+, Xcode Command Line Tools (macOS).

```bash
./scripts/check.sh              # hard-code gate, fmt, clippy, tests, UI typecheck
npm --prefix ui run tauri dev   # run the app
npm --prefix ui run tauri build # build the macOS .app/.dmg
```

## Project governance

- Constitution: `.specify/memory/constitution.md`
- Workflow: `CONTRIBUTING.md` (Spec Kit lifecycle, PR-only to `main`, review gates)
