import { invoke } from "@tauri-apps/api/core";

export type Category = "A" | "B" | "C";
export type Action = "Trash" | "Permanent";
export interface ScanItem { id: string; ruleId: string; category: Category; label: string; sizeBytes: number; selectable: boolean; preselected: boolean; permanentOnly: boolean; guidance?: string; projectPath?: string; projectAgeDays?: number; duplicateGroup?: string; idleDays?: number; details: string[]; }
export interface Settings { projectRoots: string[]; largeFileThresholdBytes: number; reviewRoots: string[]; duplicateMinBytes: number; }
export type StartupScope = "userAgent" | "systemAgent" | "systemDaemon";
export interface StartupItem { label: string; plist: string; program?: string; scope: StartupScope; enabled: boolean; editable: boolean; }
export interface Scan { id: string; status: "Complete" | "Cancelled" | "Failed"; items: ScanItem[]; unscanned: { path: string; reason: string }[]; }
export interface ScanStarted { scanId: string; }
export interface Volume { totalBytes: number; usedBytes: number; freeBytes: number; }
export interface PreviewItem { itemId: string; label: string; sizeBytes: number; action: Action; details: string[]; }
export interface Preview { id: string; totalBytes: number; requiresIrreversibleAck: boolean; items: PreviewItem[]; refused: { itemId: string; reason: string }[]; }
export interface CleanupResult { label: string; sizeBytes: number; outcome: "Succeeded" | "Skipped"; reason?: string; }
export interface CleanupRun { id: string; freedBytes: number; results: CleanupResult[]; }
export interface CleanupProgress { runId: string; done: number; total: number; current: CleanupResult; }
export const api = {
  volume: () => invoke<Volume>("get_volume_summary"),
  getSettings: () => invoke<Settings>("get_settings"),
  setSettings: (settings: Settings) => invoke<Settings>("set_settings", { settings }),
  startScan: () => invoke<ScanStarted>("start_scan"),
  startAppScan: () => invoke<ScanStarted>("start_app_scan"),
  startupItems: () => invoke<StartupItem[]>("list_startup_items"),
  setStartupEnabled: (label: string, enabled: boolean, acknowledged: boolean) => invoke<StartupItem>("set_startup_item_enabled", { label, enabled, acknowledged }),
  cancelScan: (scanId: string) => invoke<void>("cancel_scan", { scanId }),
  getScan: (scanId: string) => invoke<Scan>("get_scan", { scanId }),
  preview: (scanId: string, itemIds: string[], permanent: boolean) => invoke<Preview>("preview_cleanup", { scanId, itemIds, permanent }),
  run: (previewId: string, acknowledgeIrreversible: boolean) => invoke<CleanupRun>("run_cleanup", { previewId, acknowledgeIrreversible }),
};
export const bytes = (value: number) => new Intl.NumberFormat(undefined, { style: "unit", unit: "byte", notation: "compact", unitDisplay: "narrow" }).format(value);
