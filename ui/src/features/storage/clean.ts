import { api, bytes, type CleanupRun, type ScanItem } from "../../api";

const escapeHtml = (text: string) => text.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);

/** Preview → confirm → run. The backend preview is authoritative: it shows every
 *  item with its action, refusals, and whether an irreversible acknowledgement is needed. */
export async function confirmCleanup(scanId: string, selected: ScanItem[], options: { allowPermanent?: boolean } = {}): Promise<CleanupRun | undefined> {
  const allowPermanent = options.allowPermanent ?? true;
  let permanentChosen = false;
  let preview = await api.preview(scanId, selected.map((item) => item.id), permanentChosen);
  const dialog = document.createElement("dialog");
  dialog.className = "dialog";
  const render = () => {
    const refused = preview.refused.length ? `<p>Not included (${preview.refused.length}): ${preview.refused.map((r) => `${escapeHtml(selected.find((s) => s.id === r.itemId)?.label ?? r.itemId)} — ${escapeHtml(r.reason)}`).join("; ")}</p>` : "";
    const list = preview.items.map((item) => `<li>${escapeHtml(item.label)} · ${bytes(item.sizeBytes)} · <strong>${item.action === "Permanent" ? "Delete permanently" : "Move to Trash"}</strong>${item.details.length ? `<details><summary>${item.details.length} detail(s)</summary><ul>${item.details.slice(0, 200).map((d) => `<li><code>${escapeHtml(d)}</code></li>`).join("")}</ul></details>` : ""}</li>`).join("");
    dialog.innerHTML = `<h2>Review cleanup</h2><p>${preview.items.length} item(s), ${bytes(preview.totalBytes)}</p><ul>${list}</ul>${refused}${allowPermanent ? `<label><input id="permanent" type="checkbox"> Permanently delete instead of Trash (irreversible)</label>` : ""}<label id="ack" ${preview.requiresIrreversibleAck ? "" : "hidden"}><input type="checkbox"> I understand this cannot be undone</label><form method="dialog"><button value="cancel">Cancel</button><button class="primary" value="confirm" ${preview.items.length ? "" : "disabled"}>Confirm</button></form>`;
    const permanent = dialog.querySelector<HTMLInputElement>("#permanent");
    if (permanent) {
      permanent.checked = permanentChosen;
      // A new preview is requested so the backend decides each item's action.
      permanent.onchange = async () => { permanentChosen = permanent.checked; preview = await api.preview(scanId, selected.map((item) => item.id), permanentChosen); render(); };
    }
  };
  render();
  document.body.append(dialog);
  dialog.showModal();
  await new Promise<void>((resolve) => dialog.addEventListener("close", () => resolve(), { once: true }));
  let run: CleanupRun | undefined;
  try {
    if (dialog.returnValue === "confirm") {
      const acknowledged = !preview.requiresIrreversibleAck || dialog.querySelector<HTMLInputElement>("#ack input")!.checked;
      if (!acknowledged) throw new Error("Permanent cleanup requires acknowledgement");
      run = await api.run(preview.id, acknowledged);
    }
  } finally { dialog.remove(); }
  return run;
}
