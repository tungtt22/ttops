import { api, bytes, type ScanItem } from "../../api";

export async function confirmCleanup(scanId: string, selected: ScanItem[]): Promise<void> {
  const dialog = document.createElement("dialog");
  dialog.className = "dialog";
  const permanent = selected.some((item) => item.permanentOnly);
  dialog.innerHTML = `<h2>Review cleanup</h2><p>${selected.length} item(s), ${bytes(selected.reduce((total, item) => total + item.sizeBytes, 0))}</p><ul>${selected.map((item) => `<li>${item.label}</li>`).join("")}</ul><label><input id="permanent" type="checkbox" ${permanent ? "checked disabled" : ""}> Permanently delete (irreversible)</label><label id="ack" hidden><input type="checkbox"> I understand this cannot be undone</label><form method="dialog"><button value="cancel">Cancel</button><button class="primary" value="confirm">Confirm</button></form>`;
  document.body.append(dialog);
  const permanentInput = dialog.querySelector<HTMLInputElement>("#permanent")!;
  const acknowledgement = dialog.querySelector<HTMLElement>("#ack")!;
  permanentInput.onchange = () => { acknowledgement.hidden = !permanentInput.checked; };
  if (permanent) acknowledgement.hidden = false;
  dialog.showModal();
  await new Promise<void>((resolve) => dialog.addEventListener("close", () => resolve(), { once: true }));
  if (dialog.returnValue === "confirm") {
    const preview = await api.preview(scanId, selected.map((item) => item.id), permanentInput.checked);
    const acknowledged = !preview.requiresIrreversibleAck || dialog.querySelector<HTMLInputElement>("#ack input")!.checked;
    if (!acknowledged) throw new Error("Permanent cleanup requires acknowledgement");
    await api.run(preview.id, acknowledged);
  }
  dialog.remove();
}
