import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { api, bytes, type Scan, type ScanItem } from "../../api";
import { confirmCleanup } from "../storage/clean";

let unlisten: UnlistenFn | undefined;

/** App Uninstaller: one app (with its leftovers) per row; always moved to the Trash. */
export async function appsView(root: HTMLElement): Promise<void> {
  root.innerHTML = `<section><h1>Apps</h1><p>Uninstall apps together with their caches, preferences, containers and logs. Everything goes to the Trash, so you can restore it.</p><button class="primary" id="scan">Scan apps</button><input id="filter" placeholder="Filter by name" size="24"><p id="status"></p><div id="results"></div></section>`;
  const status = root.querySelector<HTMLElement>("#status")!;
  const results = root.querySelector<HTMLElement>("#results")!;
  const filter = root.querySelector<HTMLInputElement>("#filter")!;
  const scanButton = root.querySelector<HTMLButtonElement>("#scan")!;
  let activeScanId: string | undefined;
  let scan: Scan | undefined;

  const draw = () => {
    if (!scan) return;
    const term = filter.value.trim().toLowerCase();
    const apps = scan.items.filter((item) => item.ruleId === "app-uninstall" && item.label.toLowerCase().includes(term)).sort((a, b) => b.sizeBytes - a.sizeBytes);
    const orphans = scan.items.filter((item) => item.ruleId === "app-orphans" && item.label.toLowerCase().includes(term)).sort((a, b) => b.sizeBytes - a.sizeBytes);
    results.replaceChildren(section("Installed apps", apps, true), section("Leftovers of removed apps", orphans, false));
  };
  const section = (title: string, items: ScanItem[], single: boolean) => {
    const card = document.createElement("section"); card.className = "card";
    const heading = document.createElement("h2"); heading.textContent = `${title} · ${items.length}`; card.append(heading);
    if (!items.length) { const p = document.createElement("p"); p.textContent = "Nothing found."; card.append(p); return card; }
    const boxes: [HTMLInputElement, ScanItem][] = [];
    for (const item of items) {
      const label = document.createElement("label"); label.className = "item";
      const box = document.createElement("input"); box.type = "checkbox"; box.disabled = !item.selectable;
      boxes.push([box, item]);
      const leftovers = Math.max(0, item.details.length - 1);
      label.append(box, document.createTextNode(`${item.label} (${bytes(item.sizeBytes)})${single ? ` — ${leftovers} leftover item(s)` : ""}${item.guidance ? ` — ${item.guidance}` : ""}`));
      card.append(label);
    }
    const action = document.createElement("button"); action.className = "primary"; action.type = "button";
    action.textContent = single ? "Uninstall selected" : "Move selected to Trash";
    action.onclick = async () => {
      const selected = boxes.filter(([box]) => box.checked).map(([, item]) => item);
      if (!selected.length || !scan) return;
      try {
        const run = await confirmCleanup(scan.id, selected, { allowPermanent: false });
        if (!run) return;
        const skipped = run.results.filter((r) => r.outcome === "Skipped");
        status.textContent = `Moved to Trash: ${bytes(run.freedBytes)}; ${run.results.length - skipped.length} item(s) done; ${skipped.length} skipped${skipped.length ? ` (${skipped.map((r) => `${r.label}: ${r.reason ?? "failed"}`).join(", ")})` : ""}. Scan again to refresh.`;
      } catch (error) { status.textContent = String(error); }
    };
    card.append(action);
    return card;
  };

  unlisten?.();
  unlisten = await listen<{ scanId: string }>("scan://finished", async ({ payload }) => {
    if (payload.scanId !== activeScanId) return;
    activeScanId = undefined; scanButton.disabled = false;
    try { scan = await api.getScan(payload.scanId); status.textContent = `${scan.items.filter((i) => i.ruleId === "app-uninstall").length} apps found`; draw(); }
    catch (error) { status.textContent = String(error); }
  });
  filter.oninput = draw;
  scanButton.onclick = async () => {
    scanButton.disabled = true; status.textContent = "Scanning apps (read-only)…";
    try { activeScanId = (await api.startAppScan()).scanId; }
    catch (error) { status.textContent = String(error); scanButton.disabled = false; }
  };
}
