# Feature Specification: Mac Maintenance Toolkit

**Feature Branch**: `002-mac-maintenance` (stacked on `001-storage-bc`)
**Created**: 2026-11-27
**Status**: Draft
**Input**: Owner request: add maintenance features comparable to commercial Mac cleaners to TTops —
app uninstaller, extended system junk, duplicate and old files, startup items, and developer
toolchain cleanup (Homebrew, programming-language package caches, Docker). Implemented natively in
the TTops Rust core; no third-party application code is reused.

Owner decisions (2026-11-27): constitution 1.2.0 App Uninstaller exception (Trash only); external
tool cleanups allowed as permanent with dry-run preview and double confirmation; startup items:
view all, toggle only the current user's LaunchAgents; macOS first.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Uninstall an app with its leftovers (Priority: P1)

The user opens Apps, sees installed apps with size (bundle + leftovers), selects one, reviews the
exact files (bundle, caches, preferences, containers, logs, saved state, launch agents) and moves
them all to the Trash.

**Independent Test**: seed a fake applications folder and library with an app and leftovers in a
temp dir; uninstall; verify the bundle and leftovers are in the test Trash and unrelated files remain.

**Acceptance Scenarios**:
1. **Given** the Apps view, **When** it scans, **Then** every bundle directly inside the platform
   applications folders is listed with name, bundle id, bundle size and leftover size; Apple system
   apps (`com.apple.*`) are shown as not removable.
2. **Given** a selected app, **When** previewed, **Then** the preview lists the bundle and every
   leftover path, and the action is always "Move to Trash", even if permanent delete is requested.
3. **Given** the app is running, **When** the user runs the uninstall, **Then** the app is skipped
   with reason "in use" and nothing is moved.
4. **Given** a leftover inside a protected location, **When** run, **Then** that leftover is skipped
   as protected and the rest continue.
5. **Given** library folders named after a bundle id with no installed app, **When** scanned,
   **Then** they are listed as orphaned leftovers (not preselected, never `com.apple.*`).

### User Story 2 - Clean extended system and developer junk (Priority: P1)

Storage scan also finds user logs, Xcode DerivedData and device support files, more package
manager caches (yarn, pnpm, bun, Go modules, CocoaPods, Composer), Homebrew cleanup and unused
formulae, and Docker dangling images and stopped containers.

**Acceptance Scenarios**:
1. **Given** a scan, **Then** regenerable caches appear in category A and are preselected, except
   external tool cleanups, which are never preselected.
2. **Given** Homebrew is installed, **When** scanned, **Then** "Homebrew cleanup" shows the size from
   `brew cleanup --dry-run`, and "Unused formulae" lists `brew autoremove --dry-run` names.
3. **Given** an external tool item is selected, **When** previewed, **Then** the action is permanent
   and the irreversible acknowledgement is required.
4. **Given** Docker, **Then** dangling images (B) and stopped containers (C) are listed; unused
   volumes are review-only with guidance; running containers are never touched.
5. **Given** iOS device backups or Xcode archives, **Then** they are listed in C, never preselected.

### User Story 3 - Find duplicate and idle large files (Priority: P2)

The user adds review folders. The scan groups identical files (same size and content). The user
can remove copies but never every copy of a group. Large files show days since last use.

**Acceptance Scenarios**:
1. **Given** review folders with identical files, **Then** each group lists all copies in C, none
   preselected.
2. **Given** every copy of a group is selected, **When** previewed, **Then** one copy is refused with
   reason "keep one copy".
3. **Given** large files, **Then** each shows days since last use (newest of access and modify time).

### User Story 4 - Review and toggle startup items (Priority: P2)

The user opens Startup and sees launch agents and daemons with label, program, scope, and state.
The user can disable or enable their own LaunchAgents after confirming; system items are read-only.

**Acceptance Scenarios**:
1. **Given** the Startup view, **Then** user agents, system agents and system daemons are listed.
2. **Given** a user agent, **When** the user disables it and confirms, **Then** TTops runs
   `launchctl disable gui/<uid>/<label>`, records an audit event, and shows the new state.
3. **Given** a system item or an unknown label, **When** a toggle is requested, **Then** it is refused.

### Edge Cases

- Homebrew or Docker missing: their items are absent, not errors.
- App bundle symlinked into the applications folder: not removable.
- Duplicate groups that span a protected file: the protected file is excluded from the group.
- Files changing between scan and run: re-checked at execution (exists, guard, running app).

## Requirements *(mandatory)*

- **FR-001**: Uninstall MUST only accept bundles that are direct, non-symlink children of the
  platform applications folders, not `com.apple.*`, not running; action is always Trash.
- **FR-002**: Leftovers MUST be matched by exact bundle id (or `<id>.plist`, `<id>.savedState`,
  `*.<id>` group containers) and each MUST pass the guard at execution.
- **FR-003**: Every other cleanup path MUST continue to refuse `.app` bundles.
- **FR-004**: External tool items MUST be permanent-only, never preselected, previewed by dry run.
- **FR-005**: Duplicate detection MUST confirm equality by full content comparison, not only hash.
- **FR-006**: Preview MUST refuse at least one copy when all copies of a group are requested.
- **FR-007**: Startup toggles MUST be limited to the current user's LaunchAgents directory, use
  `launchctl enable|disable gui/<uid>/<label>`, validate the label, and be audited.
- **FR-008**: All scans remain read-only and cancellable; tests use temp dirs and fakes only.
- **FR-009**: Platform gap: Windows has no uninstaller or startup view in this feature; duplicate,
  idle-file and package-cache scanning are shared.

## Success Criteria *(mandatory)*

- **SC-001**: Uninstall of a seeded app moves the bundle and all seeded leftovers to the test Trash.
- **SC-002**: No test or code path can remove an Apple app, a running app, or the last duplicate.
- **SC-003**: All quality gates pass (`scripts/check.sh`).

## Assumptions

- Login items registered through Background Task Management cannot be listed without elevated
  rights; the Startup view links the user to System Settings for them.
- Unused app localizations are out of scope: removing them modifies signed bundles.
