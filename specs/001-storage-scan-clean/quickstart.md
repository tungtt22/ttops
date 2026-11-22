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
- `scanner` — seeded temp home: items/categories/sizes match; before/after snapshot identical (SC-004);
  cancel stops and marks incomplete.
- `cleaner` — preview required; reused preview rejected; permanent needs ack; vanished/locked items
  skipped with reason; trash mode moves into the fake platform's trash dir.
- `audit` — run without `runFinished` reads back as `interrupted` (SC-006).

## Run the app

```bash
cd ui && npm install && cd ..
npm --prefix ui run tauri dev
```

Manual checks:

1. Storage view shows total/used/free of the home volume (FR-001).
2. Click **Scan**; categories A/B/C fill in with sizes; Cancel works (US1).
3. Only A items are ticked; open **Clean** → preview lists items, total, "Move to Trash" (US2).
4. Toggle permanent → second confirmation appears.
5. After cleanup, result shows freed and skipped items; **History** shows the run (US5).
6. Add a project root under category B; build folders appear with age (US3).

## Build release

```bash
npm --prefix ui run tauri build     # .app/.dmg on macOS, .msi on Windows
```
