# Quickstart: Storage Scan and Clean

## Prerequisites

- Rust stable (1.97+), Node 24+, npm
- macOS: Xcode Command Line Tools. Windows: WebView2 runtime, MSVC build tools.
- Optional: Docker or OrbStack CLI for container items.

## Validate the core (safe — temp dirs only)

```bash
./scripts/check.sh          # fmt --check, clippy -D warnings, cargo test, tsc --noEmit
```

Expected: all pass. Key suites:

- `guard` — protected roots, link escapes, credential name patterns are refused (SC-003).
- `scanner` — seeded temp home: macOS category A items/sizes match; before/after snapshot identical
  (SC-004); cancellation during a representative large tree stops and marks the scan incomplete.
- `cleaner` — preview required; reused preview rejected; permanent needs ack; vanished/locked items
  skipped with reason; protected children are skipped while safe siblings continue; trash mode moves
  into the fake platform's trash dir.
- `audit` — run without `runFinished` reads back as `interrupted` (SC-006).

## Run the app

```bash
cd ui && npm install && cd ..
npm --prefix ui run tauri dev
```

Manual checks:

1. Storage view shows total/used/free of the home volume (FR-001).
2. Click **Scan**; the macOS category A safe-cache results fill in with sizes; Cancel works (US1).
3. Only A items are ticked; open **Clean** → preview lists items, total, "Move to Trash" (US2).
4. Toggle permanent → second confirmation appears.
5. After cleanup, per-item progress appears and the final result shows freed bytes, succeeded items,
   and skipped items with reasons (US2).
6. Category B project-output scanning, category C review-only data, Windows support, and persisted
   History are deferred to v0.2.0.

## Build release

```bash
npm --prefix ui run tauri build     # .app/.dmg on macOS, .msi on Windows
```

## macOS validation record (2026-11-22)

- `./scripts/check.sh` passed: hard-code gate, Rust formatting, clippy with warnings denied,
  workspace tests, and TypeScript typecheck. Core tests use only `tempfile` fixtures.
- `npm --prefix ui run tauri build` produced `TTops.app` and
  `TTops_0.1.0_aarch64.dmg` on macOS. The manual GUI checklist above remains for the next
  interactive validation pass; this automated run did not execute cleanup against machine data
  or the real Trash.
