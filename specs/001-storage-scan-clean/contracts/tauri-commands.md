# Contract: Tauri commands and events (UI ↔ core)

Types refer to [data-model.md](../data-model.md). Errors are returned as
`{ code: string, message: string }`.

## Commands

| Command | Args | Returns | Notes |
|---------|------|---------|-------|
| `get_volume_summary` | – | `VolumeSummary` | |
| `start_scan` | – | `{ scanId }` | rejects with `scanRunning` if one is active; progress via events |
| `cancel_scan` | `{ scanId }` | – | stops within 2 s; session status `cancelled` |
| `get_scan` | `{ scanId }` | `ScanSession` | |
| `preview_cleanup` | `{ scanId, itemIds: string[], permanent: bool }` | `CleanupPreview` | no side effects |
| `run_cleanup` | `{ previewId, acknowledgeIrreversible: bool }` | `CleanupRun` (including `freedBytes` and per-item `results`) | errors: `unknownPreview`, `previewUsed`, `ackRequired`, `cleanupRunning` |
| `list_runs` | – | `CleanupRunSummary[]` (newest first) | summary = run without `results` + counts |
| `get_run` | `{ runId }` | `CleanupRun` | |
| `get_settings` | – | `Settings` | |
| `set_settings` | `Settings` | `Settings` | project roots must be existing directories and not protected |
| `pick_project_root` | – | `string?` | native folder dialog |

## Events

| Event | Payload |
|-------|---------|
| `scan://progress` | `{ scanId, currentRule, itemsFound, bytesFound }` (≤ 10/s) |
| `scan://finished` | `{ scanId, status }` |
| `cleanup://progress` | `{ runId, done, total, current: ItemResult }` |
| `cleanup://finished` | `{ runId, freedBytes, succeeded, skipped }` |

## Invariants (tested)

1. No command other than `run_cleanup` changes filesystem or container state.
2. `run_cleanup` acts only on items in the referenced preview, at most once per preview.
3. Permanent mode without `acknowledgeIrreversible: true` changes nothing.
4. Every guard-refused path is reported as `protected` and left untouched.
