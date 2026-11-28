import { listen } from "@tauri-apps/api/event";
import { api, bytes, type Scan, type ScanItem, type Settings } from "../../api";
import { confirmCleanup } from "./clean";

interface ScanFinished { scanId: string; status: Scan["status"]; }

export async function storageView(root: HTMLElement): Promise<void> {
  root.innerHTML = `<section><h1>Storage</h1><p id="volume">Loading volume…</p><button class="primary" id="scan">Scan</button><button id="cancel" hidden disabled>Cancel</button><button id="clean" disabled>Clean selected</button><p id="status"></p><div id="settings" class="card"></div><div id="results"></div></section>`;
  await settingsPanel(root.querySelector<HTMLElement>("#settings")!);
  try { const volume = await api.volume(); root.querySelector("#volume")!.textContent = `${bytes(volume.usedBytes)} used of ${bytes(volume.totalBytes)} · ${bytes(volume.freeBytes)} free`; } catch { root.querySelector("#volume")!.textContent = "Volume summary unavailable"; }

  const scanButton = root.querySelector<HTMLButtonElement>("#scan")!;
  const cancelButton = root.querySelector<HTMLButtonElement>("#cancel")!;
  let activeScanId: string | undefined;
  let earlyFinishedScanId: string | undefined;
  const finishScan = async (scanId: string) => {
    activeScanId = undefined;
    cancelButton.hidden = true;
    cancelButton.disabled = true;
    scanButton.disabled = false;
    try { render(root, await api.getScan(scanId)); } catch (error) { root.querySelector("#status")!.textContent = String(error); }
  };
  await listen<ScanFinished>("scan://finished", ({ payload }) => {
    if (payload.scanId === activeScanId) void finishScan(payload.scanId);
    else earlyFinishedScanId = payload.scanId;
  });
  await listen<{ done: number; total: number; current: { label: string } }>("cleanup://progress", ({ payload }) => {
    root.querySelector("#status")!.textContent = `Cleaning ${payload.done}/${payload.total}: ${payload.current.label}`;
  });

  scanButton.onclick = async () => {
    scanButton.disabled = true;
    root.querySelector("#status")!.textContent = "Scanning read-only locations…";
    try {
      const started = await api.startScan();
      activeScanId = started.scanId;
      cancelButton.hidden = false;
      cancelButton.disabled = false;
      if (earlyFinishedScanId === activeScanId) void finishScan(activeScanId);
    } catch (error) {
      root.querySelector("#status")!.textContent = String(error);
      scanButton.disabled = false;
    }
  };
  cancelButton.onclick = async () => {
    if (!activeScanId) return;
    cancelButton.disabled = true;
    root.querySelector("#status")!.textContent = "Cancelling scan…";
    try { await api.cancelScan(activeScanId); } catch (error) { root.querySelector("#status")!.textContent = String(error); cancelButton.disabled = false; }
  };
}
export function render(root: HTMLElement, scan: Scan): void {
  root.querySelector("#status")!.textContent = scan.status === "Cancelled" ? "Incomplete scan — partial results retained" : "Scan complete";
  const groups: Record<string, ScanItem[]> = { A: [], B: [], C: [] };
  scan.items.forEach((item) => groups[item.category].push(item));
  const results = root.querySelector("#results")!;
  const clean = root.querySelector<HTMLButtonElement>("#clean")!;
  const selectedItems = () => scan.items.filter((item) => item.selectable && results.querySelector<HTMLInputElement>(`input[data-id="${CSS.escape(item.id)}"]`)?.checked);
  // Recomputed after every checkbox or bulk change, for every category.
  const updateClean = () => { clean.disabled = selectedItems().length === 0; };
  results.replaceChildren(...Object.entries(groups).map(([category, items]) => group(category, items, updateClean)));
  updateClean();
  clean.onclick = async () => {
    const selected = selectedItems();
    try {
      const run = await confirmCleanup(scan.id, selected);
      if (!run) return;
      const skipped = run.results.filter((result) => result.outcome === "Skipped");
      root.querySelector("#status")!.textContent = `Cleanup complete: ${bytes(run.freedBytes)} freed; ${run.results.length - skipped.length} succeeded; ${skipped.length} skipped${skipped.length ? ` (${skipped.map((result) => `${result.label}: ${result.reason ?? "failed"}`).join(", ")})` : ""}`;
    } catch (error) { root.querySelector("#status")!.textContent = String(error); }
  };
}
const TITLES: Record<string, string> = { A: "Safe caches (A)", B: "Developer build outputs (B)", C: "Large data to review (C)" };

function describe(item: ScanItem): string {
  const parts = [`${item.label} (${bytes(item.sizeBytes)})`];
  if (item.projectAgeDays !== undefined && item.projectAgeDays !== null) parts.push(`project untouched ${item.projectAgeDays} day(s)`);
  if (item.guidance) parts.push(item.guidance);
  return parts.join(" — ");
}

function group(category: string, items: ScanItem[], onSelectionChange: () => void): HTMLElement {
  const section = document.createElement("section");
  section.className = "card";
  const total = items.reduce((sum, item) => sum + item.sizeBytes, 0);
  const heading = document.createElement("h2"); heading.textContent = `${TITLES[category]} · ${bytes(total)}`; section.append(heading);
  if (!items.length) {
    const empty = document.createElement("p");
    empty.textContent = category === "B" ? "No build outputs found. Add a project folder above, then scan again." : "No items found.";
    section.append(empty);
    return section;
  }
  const boxes: HTMLInputElement[] = [];
  if (category === "C") {
    const note = document.createElement("p"); note.textContent = "Personal data: review and select items one by one."; section.append(note);
  } else {
    // Category C is never part of "select all".
    const all = document.createElement("button"); all.textContent = "Select all"; all.type = "button";
    all.onclick = () => { boxes.forEach((box) => { if (!box.disabled) box.checked = true; }); onSelectionChange(); };
    const none = document.createElement("button"); none.textContent = "Select none"; none.type = "button";
    none.onclick = () => { boxes.forEach((box) => { box.checked = false; }); onSelectionChange(); };
    section.append(all, none);
  }
  for (const item of items) {
    const label = document.createElement("label"); label.className = "item";
    const checkbox = document.createElement("input"); checkbox.type = "checkbox"; checkbox.dataset.id = item.id; checkbox.checked = item.preselected; checkbox.disabled = !item.selectable; checkbox.onchange = onSelectionChange;
    boxes.push(checkbox);
    label.append(checkbox, document.createTextNode(describe(item))); section.append(label);
  }
  return section;
}

async function settingsPanel(panel: HTMLElement): Promise<void> {
  let settings: Settings;
  try { settings = await api.getSettings(); } catch (error) { panel.textContent = `Settings unavailable: ${String(error)}`; return; }
  const draw = (message = "") => {
    panel.replaceChildren();
    const heading = document.createElement("h2"); heading.textContent = "Scan settings"; panel.append(heading);
    const rootsTitle = document.createElement("p"); rootsTitle.textContent = settings.projectRoots.length ? "Project folders (category B):" : "No project folders yet. Add one to find build outputs (category B)."; panel.append(rootsTitle);
    for (const root of settings.projectRoots) {
      const row = document.createElement("div"); row.className = "item";
      const remove = document.createElement("button"); remove.type = "button"; remove.textContent = "Remove";
      remove.onclick = () => void save({ ...settings, projectRoots: settings.projectRoots.filter((other) => other !== root) });
      row.append(document.createTextNode(root + " "), remove); panel.append(row);
    }
    const input = document.createElement("input"); input.placeholder = "Absolute folder path to your projects"; input.size = 48;
    const add = document.createElement("button"); add.type = "button"; add.textContent = "Add folder";
    add.onclick = () => { const value = input.value.trim(); if (value) void save({ ...settings, projectRoots: [...settings.projectRoots, value] }); };
    const threshold = document.createElement("input"); threshold.type = "number"; threshold.min = "0.1"; threshold.step = "0.1";
    threshold.value = String(+(settings.largeFileThresholdBytes / 2 ** 30).toFixed(2));
    threshold.onchange = () => void save({ ...settings, largeFileThresholdBytes: Math.round(Number(threshold.value) * 2 ** 30) });
    const thresholdLabel = document.createElement("label"); thresholdLabel.append("Large file threshold (GB, category C): ", threshold);
    const status = document.createElement("p"); status.textContent = message;
    panel.append(input, add, thresholdLabel, status);
  };
  const save = async (next: Settings) => {
    try { settings = await api.setSettings(next); draw("Saved. Scan again to update results."); }
    catch (error) { draw(`Not saved: ${String(error)}`); }
  };
  draw();
}
