import { api, bytes, type CleanupRun, type ScanItem } from "../../api";

function el<K extends keyof HTMLElementTagNameMap>(tag: K, text?: string): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (text !== undefined) node.textContent = text;
  return node;
}

function checkboxLabel(text: string): { label: HTMLLabelElement; input: HTMLInputElement } {
  const label = el("label");
  const input = el("input");
  input.type = "checkbox";
  label.append(input, ` ${text}`);
  return { label, input };
}

export interface CleanupDialog {
  dialog: HTMLDialogElement;
  permanentInput: HTMLInputElement;
  ackInput: HTMLInputElement;
}

/**
 * Builds the cleanup preview with DOM nodes only. Scan item labels come from
 * the filesystem, so they are set through `textContent` and never parsed as HTML.
 */
export function buildCleanupDialog(selected: ScanItem[]): CleanupDialog {
  const dialog = el("dialog");
  dialog.className = "dialog";
  const permanent = selected.some((item) => item.permanentOnly);
  const total = selected.reduce((sum, item) => sum + item.sizeBytes, 0);

  const list = el("ul");
  for (const item of selected) list.append(el("li", item.label));

  const permanentOption = checkboxLabel("Permanently delete (irreversible)");
  permanentOption.input.id = "permanent";
  permanentOption.input.checked = permanent;
  permanentOption.input.disabled = permanent;

  const ack = checkboxLabel("I understand this cannot be undone");
  ack.label.id = "ack";
  ack.label.hidden = !permanent;
  permanentOption.input.onchange = () => { ack.label.hidden = !permanentOption.input.checked; };

  const form = el("form");
  form.method = "dialog";
  const cancel = el("button", "Cancel");
  cancel.value = "cancel";
  const confirm = el("button", "Confirm");
  confirm.className = "primary";
  confirm.value = "confirm";
  form.append(cancel, confirm);

  dialog.append(
    el("h2", "Review cleanup"),
    el("p", `${selected.length} item(s), ${bytes(total)}`),
    list,
    permanentOption.label,
    ack.label,
    form,
  );
  return { dialog, permanentInput: permanentOption.input, ackInput: ack.input };
}

export async function confirmCleanup(scanId: string, selected: ScanItem[]): Promise<CleanupRun | undefined> {
  const { dialog, permanentInput, ackInput } = buildCleanupDialog(selected);
  document.body.append(dialog);
  dialog.showModal();
  await new Promise<void>((resolve) => dialog.addEventListener("close", () => resolve(), { once: true }));
  let run: CleanupRun | undefined;
  if (dialog.returnValue === "confirm") {
    const preview = await api.preview(scanId, selected.map((item) => item.id), permanentInput.checked);
    const acknowledged = !preview.requiresIrreversibleAck || ackInput.checked;
    if (!acknowledged) throw new Error("Permanent cleanup requires acknowledgement");
    run = await api.run(preview.id, acknowledged);
  }
  dialog.remove();
  return run;
}
