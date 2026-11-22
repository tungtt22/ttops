import { invoke } from "@tauri-apps/api/core";

export type Category = "A" | "B" | "C";
export type Action = "Trash" | "Permanent";
export interface ScanItem { id: string; ruleId: string; category: Category; label: string; sizeBytes: number; selectable: boolean; preselected: boolean; permanentOnly: boolean; guidance?: string; }
export interface Scan { id: string; status: "Complete" | "Cancelled" | "Failed"; items: ScanItem[]; unscanned: { path: string; reason: string }[]; }
export interface ScanStarted { scanId: string; }
export interface Volume { totalBytes: number; usedBytes: number; freeBytes: number; }
export interface Preview { id: string; totalBytes: number; requiresIrreversibleAck: boolean; items: ScanItem[]; }
export const api = {
  volume: () => invoke<Volume>("get_volume_summary"),
  startScan: () => invoke<ScanStarted>("start_scan"),
  cancelScan: (scanId: string) => invoke<void>("cancel_scan", { scanId }),
  getScan: (scanId: string) => invoke<Scan>("get_scan", { scanId }),
  preview: (scanId: string, itemIds: string[], permanent: boolean) => invoke<Preview>("preview_cleanup", { scanId, itemIds, permanent }),
  run: (previewId: string, acknowledgeIrreversible: boolean) => invoke("run_cleanup", { previewId, acknowledgeIrreversible }),
};
export const bytes = (value: number) => new Intl.NumberFormat(undefined, { style: "unit", unit: "byte", notation: "compact", unitDisplay: "narrow" }).format(value);
