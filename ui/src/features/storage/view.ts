import { listen } from "@tauri-apps/api/event";
import { api, bytes, type Scan, type ScanItem } from "../../api";
import { confirmCleanup } from "./clean";

interface ScanFinished { scanId: string; status: Scan["status"]; }

export async function storageView(root: HTMLElement): Promise<void> {
  root.innerHTML = `<section><h1>Storage</h1><p id="volume">Loading volume…</p><button class="primary" id="scan">Scan</button><button id="cancel" hidden disabled>Cancel</button><button id="clean" disabled>Clean selected</button><p id="status"></p><div id="results"></div></section>`;
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
function render(root: HTMLElement, scan: Scan): void {
  root.querySelector("#status")!.textContent = scan.status === "Cancelled" ? "Incomplete scan — partial results retained" : "Scan complete";
  const groups: Record<string, ScanItem[]> = { A: [], B: [], C: [] };
  scan.items.forEach((item) => groups[item.category].push(item));
  const results = root.querySelector("#results")!;
  results.replaceChildren(...Object.entries(groups).map(([category, items]) => group(category, items)));
  const clean = root.querySelector<HTMLButtonElement>("#clean")!;
  clean.disabled = !scan.items.some((item) => item.preselected && item.selectable);
  clean.onclick = async () => {
    const selected = scan.items.filter((item) => results.querySelector<HTMLInputElement>(`input[data-id="${CSS.escape(item.id)}"]`)?.checked);
    try { await confirmCleanup(scan.id, selected); root.querySelector("#status")!.textContent = "Cleanup completed"; } catch (error) { root.querySelector("#status")!.textContent = String(error); }
  };
}
function group(category: string, items: ScanItem[]): HTMLElement {
  const section = document.createElement("section");
  section.className = "card";
  const heading = document.createElement("h2"); heading.textContent = `Category ${category}`; section.append(heading);
  if (!items.length) { const empty = document.createElement("p"); empty.textContent = "No items found."; section.append(empty); return section; }
  for (const item of items) {
    const label = document.createElement("label"); label.className = "item";
    const checkbox = document.createElement("input"); checkbox.type = "checkbox"; checkbox.dataset.id = item.id; checkbox.checked = item.preselected; checkbox.disabled = !item.selectable;
    label.append(checkbox, document.createTextNode(`${item.label} (${bytes(item.sizeBytes)})${item.guidance ? ` — ${item.guidance}` : ""}`)); section.append(label);
  }
  return section;
}
