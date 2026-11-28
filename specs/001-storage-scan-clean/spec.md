# Feature Specification: Storage Scan and Clean

**Feature Branch**: `001-storage-scan-clean`
**Created**: 2026-11-22
**Status**: Draft
**Input**: User description: "Storage scan and clean (MVP of TTops). The v0.1.0 macOS release scans
safe caches (A) only; developer build outputs (B) and large user data for review (C) are deferred to
v0.2.0. Selected items follow preview → confirm → cleanup with move-to-Trash default, per-item
results, a local audit log, protected-path refusal, and an app shell for later features."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - See disk usage and reclaimable space (Priority: P1)

A user who is low on disk space opens TTops, sees how full the system volume is, and runs a scan
that lists reclaimable space grouped by category, with the size of every item and category.
Nothing on disk changes during the scan.

**Why this priority**: Knowing where the space went is the first need and is valuable on its own;
it also feeds every later story.

**Independent Test**: On a test machine with seeded caches and project folders, run a scan and
verify the listed items and sizes match the seeded data and that no file timestamps or contents
changed.

**Acceptance Scenarios**:

1. **Given** TTops is open, **When** the Storage view loads, **Then** total, used, and free space of
   the system volume are shown.
2. **Given** a scan is started in v0.1.0, **When** it completes, **Then** items appear under Safe caches (A) with item sizes and a category total; developer build outputs (B) and review-only data (C) are deferred to v0.2.0.
3. **Given** a scan is running, **When** the user cancels it, **Then** scanning stops within 2 seconds
   and partial results are kept and labelled incomplete.
4. **Given** a scan completed, **When** any scanned item is checked, **Then** its contents and
   modification time are unchanged.

---

### User Story 2 - Clean selected items safely (Priority: P1)

The user selects items, reviews a preview showing exactly what will happen, confirms, and watches
the cleanup progress. Items go to the Trash/Recycle Bin by default, and the result shows freed
space and any skipped items with the reason.

**Why this priority**: Freeing space is the core outcome; together with Story 1 it forms the MVP.

**Independent Test**: In a seeded test environment, select category A items, confirm, and verify the
items are in the Trash/Recycle Bin, the freed total matches, and protected or locked items are
reported as skipped.

**Acceptance Scenarios**:

1. **Given** scan results, **When** the screen opens, **Then** only category A items are preselected;
   B and C items are unselected.
2. **Given** items are selected, **When** the user clicks Clean, **Then** a preview lists every item,
   the total size, and the action ("Move to Trash" by default), and nothing is changed until the user
   confirms.
3. **Given** the preview, **When** the user enables permanent delete, **Then** a second explicit
   confirmation stating the action is irreversible is required, and the choice is not remembered for
   the next run.
4. **Given** a confirmed cleanup, **When** it runs, **Then** per-item progress is shown, and at the end
   the result shows freed space, succeeded items, and skipped items with a reason (in use, locked,
   permission denied, protected, no longer exists).
5. **Given** a container-image item, **When** it is cleaned, **Then** only images not used by any
   container and not built locally are removed, and no running container is stopped.

---

### User Story 3 - Find developer build outputs in my projects (Priority: P2)

A developer adds one or more project root folders. The scan finds build output folders
(dependency folders, build targets, virtual environments, compiled output) with their size and
how long ago their project was last modified, so stale projects are easy to spot.

**Why this priority**: Large, regenerable, and common for the target user, but needs user-chosen
locations, so it follows the core flow.

**Independent Test**: Add a temporary root containing seeded projects of different ages and verify
each build output is listed with correct size and age, and none is preselected.

**Acceptance Scenarios**:

1. **Given** no project roots are configured, **When** the user opens category B, **Then** they are
   prompted to add a project root and the choice is remembered.
2. **Given** a configured root, **When** a scan runs, **Then** each build output folder appears with
   its size, its project path, and the days since the project was last modified.
3. **Given** nested build outputs (a dependency folder inside another), **When** listed, **Then**
   only the outermost folder is shown, so sizes are not counted twice.

---

### User Story 4 - Review large personal data (Priority: P2)

The user sees large items that are not regenerable caches — local AI models, files above a size
threshold, chat-app media folders — to decide manually what to remove.

**Why this priority**: Often the largest space users, but deleting them is a personal choice, so
they are shown for review only.

**Independent Test**: Seed a large file and a model folder; verify they appear in category C,
are never preselected, and chat-app media is shown with guidance rather than a delete action.

**Acceptance Scenarios**:

1. **Given** a scan, **When** results show, **Then** category C lists local model stores, files larger
   than the threshold (default 1 GB, user-adjustable), and chat-app media folders.
2. **Given** a category C item, **When** the user wants to remove it, **Then** they must select it
   individually; "select all" never includes category C.
3. **Given** a chat-app media folder, **When** shown, **Then** it is marked "clean inside the app" with
   a short instruction and cannot be deleted from TTops.

---

### User Story 5 - Review cleanup history (Priority: P3)

The user opens the History view and sees every past cleanup: when, what, how much, which action,
and the outcome per item.

**Why this priority**: Accountability and trust; not needed to free space.

**Independent Test**: Run two cleanups, then verify both appear in History with correct details and
survive an app restart.

**Acceptance Scenarios**:

1. **Given** completed cleanups, **When** History opens, **Then** runs are listed newest first with
   date, total freed, action, and counts of succeeded/skipped items.
2. **Given** a run, **When** opened, **Then** each item shows path, size, and outcome.

---

### User Story 6 - Extensible app shell (Priority: P3)

The app shows a sidebar of features (Storage, History, and later Monitor), so new tools can be
added without redesigning the app.

**Why this priority**: Enables the roadmap; delivers little value alone.

**Independent Test**: Verify the sidebar lists registered features and that a placeholder feature
can be registered and shown without changing the Storage feature.

**Acceptance Scenarios**:

1. **Given** the app starts, **When** the main window shows, **Then** a sidebar lists Storage and
   History, and selecting one shows its view.

---

### Edge Cases

- An item is deleted or changes size between scan and cleanup: re-check before acting; report
  "no longer exists" or use the current size.
- An item is in use or locked (open cache of a running browser): skip it, report "in use", and
  continue with the rest.
- No permission to read a folder during scan: skip it and list it under "not scanned" with the reason.
- The container engine is not installed or not running: show container items as unavailable,
  not as an error.
- A selected path resolves (through links) into a protected location: refuse and report "protected".
- The Trash/Recycle Bin cannot accept an item (different volume, too large): skip it with a reason
  and offer permanent delete only through the explicit opt-in flow.
- The disk becomes full or the app quits during cleanup: already-processed items are logged;
  the next launch shows the run as interrupted.
- Very large trees (millions of files): the scan shows progress and stays responsive.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST show total, used, and free space for the system volume.
- **FR-002**: The system MUST scan without creating, modifying, moving, or deleting any scanned item.
- **FR-003**: In v0.1.0, the system MUST group safe regenerable cache results into category A with
  per-item and category sizes. Categories B (developer build outputs) and C (review only) are
  deferred to v0.2.0.
- **FR-004**: Category A MUST cover, where present on the platform: browser caches, package-manager
  caches (npm, pip, uv, Homebrew, Poetry, Gradle, Maven, Cargo registry), app updater
  caches, the Trash/Recycle Bin, container build cache, and container images that are unused and
  not locally built. In v0.1.0 this catalog is macOS-only; all locations MUST resolve at runtime
  through Platform placeholders.
- **FR-005**: In v0.2.0, category B MUST find dependency folders, build targets, virtual environments, and
  compiled output folders under user-configured project roots, showing project path and days since
  last project modification, without double-counting nested folders.
- **FR-006**: In v0.2.0, category C MUST list local AI model stores, files above a user-adjustable threshold
  (default 1 GB), and chat-app media folders; chat-app media MUST be review-only with in-app guidance.
- **FR-007**: All v0.1.0 category A items MAY be preselected only when their rule declares them
  regenerable; categories B and C are not present in this release.
- **FR-008**: Before any change, the system MUST show a preview listing all items, total size, and
  action, and MUST require explicit confirmation.
- **FR-009**: The default action MUST be move to Trash/Recycle Bin. Permanent delete MUST require an
  explicit opt-in and an additional irreversible-action confirmation each run.
- **FR-010**: The system MUST refuse any action on protected paths — OS system directories,
  application bundles/program folders, credential stores, SSH/GPG keys, and cloud CLI credential
  files — including paths that resolve into them through links.
- **FR-011**: Container cleanup MUST NOT stop or remove running containers, MUST NOT remove images
  used by any container, and MUST NOT remove locally built images.
- **FR-012**: Cleanup MUST show per-item progress and finish with freed space, succeeded items, and
  skipped items with one of the reasons: in use, locked, permission denied, protected, no longer
  exists, cannot move to Trash.
- **FR-013**: Every cleanup run MUST be appended to a local audit log (time, items, sizes, action,
  per-item outcome) that survives restarts. A persisted History view is deferred to v0.2.0.
- **FR-014**: Scans MUST be cancellable; partial results MUST be labelled incomplete.
- **FR-015**: Locations that could not be scanned MUST be listed with the reason.
- **FR-016**: The app MUST present features in a sidebar driven by a feature registry, starting with
  Storage and a History placeholder; persisted History functionality is deferred to v0.2.0.
- **FR-017**: v0.1.0 MUST work on macOS. Windows support and any unavailable category items are
  deferred to v0.2.0.
- **FR-018**: The system MUST NOT send any data off the machine.

### Key Entities

- **Volume Summary**: system volume capacity, used, free.
- **Cleanup Rule**: a known location or pattern, its category, platform, whether it is regenerable,
  and whether it is preselected.
- **Scan Result Item**: path or resource identifier, category, rule, size, last modified, project path
  and age (category B), selectable flag, guidance (category C).
- **Scan Session**: start/end time, status (complete, cancelled, incomplete), items, unscanned
  locations.
- **Cleanup Run**: time, action (trash or permanent), selected items, per-item outcome and reason,
  total freed, status (completed, interrupted).
- **Project Root**: user-chosen folder scanned for category B.
- **Feature Entry**: a sidebar feature with name, icon, and view.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can go from opening the app to a completed cleanup of category A in under
  2 minutes on a typical developer machine.
- **SC-002**: A scan of v0.1.0 category A locations completes in under 3 minutes on a typical
  developer machine and the window stays responsive throughout.
- **SC-003**: In automated tests, 100% of protected-path attempts are refused, including link-based
  attempts.
- **SC-004**: 0 scanned items are modified by a scan (verified by before/after comparison in tests).
- **SC-005**: Freed space reported by TTops is within 5% of the actual change in free space.
- **SC-006**: In v0.2.0, 100% of cleanup runs, including interrupted ones, appear in History after restart.
- **SC-007**: On macOS, reproducing the owner's manual category A cleanup (caches, package managers,
  container cache and unused images, Trash) with TTops frees at least 90% of the space that the
  manual cleanup freed.

## Assumptions

- The v0.1.0 user runs TTops on macOS as a normal (non-admin) user; system-wide locations needing elevation are out
  of scope for this feature.
- The system volume is the volume holding the user's home folder.
- Container cleanup supports Docker-compatible engines (Docker Desktop, OrbStack) when their CLI is
  available; other engines are out of scope.
- "Locally built image" means an image with no registry origin (no repository digest).
- Default large-file threshold is 1 GB; project roots are empty until the user adds one.
- Scheduling, automatic cleanup, app uninstalling, and system monitoring are out of scope (the
  monitor is a later feature).
- The audit log is stored only on the local machine and kept indefinitely until the user clears it.
