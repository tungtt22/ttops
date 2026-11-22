# Data Model: Storage Scan and Clean

All types are serialised as camelCase JSON across the Tauri boundary.

## VolumeSummary

| Field | Type | Notes |
|-------|------|-------|
| mountPoint | string | volume holding the home dir |
| totalBytes / usedBytes / freeBytes | u64 | `used = total - free` |

## Rule (catalog entry)

| Field | Type | Validation |
|-------|------|------------|
| id | string | unique per catalog, kebab-case |
| name | string | non-empty |
| category | `A` \| `B` \| `C` | |
| kind | enum | `dirContents`, `dir`, `glob`, `trash`, `containerBuildCache`, `containerImages`, `projectOutputs`, `largeFiles` |
| paths | string[] | tokens expanded per platform; missing paths are skipped silently |
| regenerable | bool | `preselect` requires `regenerable` |
| preselect | bool | must be false for category B and C |
| reviewOnly | bool | true ⇒ item not selectable (e.g. chat media) |
| guidance | string? | required when `reviewOnly` |
| processNames | string[] | running process ⇒ items skipped as in use |
| permanentOnly | bool | true for `trash` kind |

Catalog load fails (test-time error) if any validation rule above is broken.

## ScanItem

| Field | Type | Notes |
|-------|------|-------|
| id | string | stable hash of `ruleId + target` |
| ruleId | string | |
| category | Category | |
| target | `{ path }` \| `{ containerImage: { id, repoTag } }` \| `{ containerBuildCache }` | |
| label | string | display name |
| sizeBytes | u64 | |
| lastModified | timestamp? | |
| projectPath / projectAgeDays | string? / u32? | category B only |
| selectable | bool | false when `reviewOnly` |
| preselected | bool | `rule.preselect && category == A` |
| permanentOnly | bool | |
| guidance | string? | |

## ScanSession

| Field | Type | Notes |
|-------|------|-------|
| id | string | |
| startedAt / finishedAt | timestamp | |
| status | `running` \| `complete` \| `cancelled` \| `failed` | `cancelled` ⇒ results labelled incomplete |
| items | ScanItem[] | |
| unscanned | `{ path, reason }[]` | FR-015 |
| containerAvailable | bool | |

State: `running → complete | cancelled | failed`.

## CleanupPreview

| Field | Type | Notes |
|-------|------|-------|
| id | string | random, single-use |
| mode | `trash` \| `permanent` | items with `permanentOnly` force `permanent` |
| items | `{ itemId, label, target, sizeBytes }[]` | non-selectable and guard-refused items are excluded and listed in `refused` |
| refused | `{ itemId, reason: "protected" \| "notSelectable" }[]` | |
| totalBytes | u64 | |
| requiresIrreversibleAck | bool | `mode == permanent` |

## CleanupRun

| Field | Type | Notes |
|-------|------|-------|
| id | string | |
| previewId | string | |
| startedAt / finishedAt | timestamp | |
| mode | `trash` \| `permanent` | |
| status | `running` \| `completed` \| `interrupted` | `interrupted` derived from log |
| results | `ItemResult[]` | |
| freedBytes | u64 | sum of succeeded sizes |

ItemResult: `{ itemId, label, target, sizeBytes, outcome: "succeeded" | "skipped", reason? }`

SkipReason: `inUse`, `locked`, `permissionDenied`, `protected`, `noLongerExists`,
`cannotMoveToTrash`, `failed` (with message).

## Settings

| Field | Type | Default |
|-------|------|---------|
| projectRoots | string[] | `[]` |
| largeFileThresholdBytes | u64 | 1 GiB |

## Audit log record (JSONL)

```text
{"type":"runStarted","runId","previewId","mode","at","items":[{itemId,label,target,sizeBytes}]}
{"type":"item","runId","itemId","outcome","reason?","sizeBytes","at"}
{"type":"runFinished","runId","freedBytes","at"}
```
