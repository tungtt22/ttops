# Research: Storage Scan and Clean

## R1. Desktop shell

- **Decision**: Tauri 2.12 (stable line); not Tauri 3 alpha.
- **Rationale**: one codebase for macOS and Windows, small binaries (system webview), Rust core.
- **Alternatives**: Electron (≈200 MB, contradicts Principle V); SwiftUI (macOS only); Tauri 3
  alpha (unstable API).

## R2. Directory walking and sizes

- **Decision**: `walkdir` with `follow_links(false)`, parallelised per rule target with `rayon`;
  size = sum of `metadata.len()` of regular files (logical size). On macOS use allocated blocks
  (`st_blocks * 512`) when available so freed space matches `df` more closely (SC-005).
- **Rationale**: `jwalk` now points users to `dua-core`; `walkdir` is stable and simple; parallelism
  at rule level is enough because rules are many and independent.
- **Alternatives**: `jwalk`, `dua-core` (heavier API), single-threaded walk (too slow for SC-002).

## R3. Trash / Recycle Bin

- **Decision**: `trash` 5 `delete_all` for move-to-Trash on both OSes. Emptying the Trash itself is
  modelled as a rule whose only allowed action is permanent delete of its contents (macOS:
  `~/.Trash/*`; Windows: `trash::os_limited::purge_all`), so it always goes through the
  irreversible-confirmation path.
- **Rationale**: well-maintained, uses Finder/Shell APIs so items can be restored.
- **Notes**: reading `~/.Trash` and some `~/Library` containers on macOS requires Full Disk Access;
  without it, the scanner reports those locations as "not scanned: permission denied" (FR-015).

## R4. Volume summary

- **Decision**: `sysinfo::Disks`; choose the disk whose mount point is the longest prefix of the
  home dir. On macOS APFS prefer `/System/Volumes/Data` when home is under it.
- **Alternatives**: `statvfs`/`GetDiskFreeSpaceEx` directly (more platform code).

## R5. Container cleanup

- **Decision**: shell out to `docker` CLI behind a `ContainerProvider` trait.
  - Build cache size: `docker system df --format json`; clean: `docker builder prune -a -f`.
  - Images: `docker images --format json` + `docker ps -a --format json` (images used by any
    container, by name or id) + `docker image inspect` (`RepoDigests` empty ⇒ locally built ⇒
    excluded). Clean: `docker rmi <id>` per image (never `-f`).
  - CLI missing or daemon down ⇒ items shown as unavailable.
- **Rationale**: works with Docker Desktop and OrbStack; no daemon API client dependency; the
  owner's manual run showed `image prune -a` would delete locally built images, hence explicit
  per-image selection.

## R6. Protected paths (guard)

- **Decision**: canonicalise the path (resolving links) and refuse if it equals or is inside a
  protected root, or matches a protected name pattern, or is the home dir / a volume root itself.
  - macOS roots: `/System`, `/bin`, `/sbin`, `/usr` (except `/usr/local` caches are not targeted
    anyway), `/Applications`, `/Library`, `~/Library/Keychains`, `~/.ssh`, `~/.gnupg`, `~/.aws`,
    `~/.config/gcloud`, `~/.azure`, `~/.kube`, `~/.docker/config.json`.
  - Windows roots: `%SystemRoot%`, `%ProgramFiles%`, `%ProgramFiles(x86)%`, `%ProgramData%`,
    `%APPDATA%\Microsoft\Credentials`, `%LOCALAPPDATA%\Microsoft\Credentials`,
    `%APPDATA%\Microsoft\Protect`, `~\.ssh`, `~\.aws`, `~\.azure`, `~\.kube`.
  - Name patterns anywhere: `*.keychain*`, `id_rsa*`, `id_ed25519*`, `*.pem`, `credentials`,
    `.env`.
  - `.app` bundles (macOS) are refused.
- **Rationale**: Principle I; the guard runs in the cleaner on every item, independent of the rule
  that produced it.

## R7. Rule catalog format

- **Decision**: embedded TOML per OS (`include_str!`), fields: `id`, `name`, `category` (A/B/C),
  `kind` (`dir_contents`, `dir`, `glob`, `container_build_cache`, `container_images`,
  `trash`, `project_outputs`, `large_files`), `paths` with tokens (`~`, `{cache}`, `{data}`,
  `{local_data}`, `%ENV%`), `regenerable`, `preselect`, `review_only`, `guidance`.
- **Rationale**: data-driven per Principle II; tests can load custom catalogs.

## R8. Cleanup safety protocol

- **Decision**: `preview_cleanup` stores an immutable preview (items, sizes, mode) under a random id;
  `run_cleanup(preview_id, acknowledge_irreversible)` executes only that preview, rejects unknown or
  reused ids, and rejects permanent mode without acknowledgement. Each item is re-stat'ed and
  re-guarded right before acting.
- **Rationale**: enforces FR-008/FR-009 in the core, not only in the UI.

## R9. Audit log

- **Decision**: JSONL file `audit.jsonl` in the app data dir; records `run_started`, `item`,
  `run_finished`. A `run_started` without `run_finished` is reported as `interrupted` (SC-006).
  Append with `O_APPEND` and flush after each record.
- **Alternatives**: SQLite (extra dependency, overkill).

## R10. "In use" detection

- **Decision**: map OS errors: Windows `ERROR_SHARING_VIOLATION`/`ERROR_LOCK_VIOLATION` ⇒ in use /
  locked; `PermissionDenied` ⇒ permission denied; `NotFound` ⇒ no longer exists; trash failure on
  cross-volume ⇒ cannot move to Trash. For browser caches, a rule may declare `process_names`;
  if such a process runs (via `sysinfo`), items are skipped as "in use".
- **Rationale**: matches the owner's manual run where an open Edge left its cache non-empty.
