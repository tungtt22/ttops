import { api, bytes, type CleanupRun, type Preview, type ScanItem } from "../../api";

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

export interface CleanupDialogParts {
  nodes: Node[];
  permanentInput?: HTMLInputElement;
  ackInput: HTMLInputElement;
}

/**
 * Builds the preview body with DOM nodes only. Labels, paths and refusal reasons
 * come from the filesystem or backend, so they are set through `textContent`
 * and never parsed as HTML.
 */
export function buildCleanupDialog(preview: Preview, selected: ScanItem[], allowPermanent: boolean, permanentChosen: boolean): CleanupDialogParts {
  const list = el("ul");
  for (const item of preview.items) {
    const entry = el("li", `${item.label} · ${bytes(item.sizeBytes)} · `);
    entry.append(el("strong", item.action === "Permanent" ? "Delete permanently" : "Move to Trash"));
    if (item.details.length) {
      const details = el("details");
      const inner = el("ul");
      for (const detail of item.details.slice(0, 200)) {
        const li = el("li");
        li.append(el("code", detail));
        inner.append(li);
      }
      details.append(el("summary", `${item.details.length} detail(s)`), inner);
      entry.append(details);
    }
    list.append(entry);
  }

  const nodes: Node[] = [el("h2", "Review cleanup"), el("p", `${preview.items.length} item(s), ${bytes(preview.totalBytes)}`), list];
  if (preview.refused.length) {
    const reasons = preview.refused.map((r) => `${selected.find((s) => s.id === r.itemId)?.label ?? r.itemId} — ${r.reason}`).join("; ");
    nodes.push(el("p", `Not included (${preview.refused.length}): ${reasons}`));
  }

  let permanentInput: HTMLInputElement | undefined;
  if (allowPermanent) {
    const option = checkboxLabel("Permanently delete instead of Trash (irreversible)");
    option.input.id = "permanent";
    option.input.checked = permanentChosen;
    permanentInput = option.input;
    nodes.push(option.label);
  }

  const ack = checkboxLabel("I understand this cannot be undone");
  ack.label.id = "ack";
  ack.label.hidden = !preview.requiresIrreversibleAck;
  nodes.push(ack.label);

  const form = el("form");
  form.method = "dialog";
  const cancel = el("button", "Cancel");
  cancel.value = "cancel";
  const confirm = el("button", "Confirm");
  confirm.className = "primary";
  confirm.value = "confirm";
  confirm.disabled = preview.items.length === 0;
  form.append(cancel, confirm);
  nodes.push(form);

  return { nodes, permanentInput, ackInput: ack.input };
}

/** Preview → confirm → run. The backend preview is authoritative: it shows every
 *  item with its action, refusals, and whether an irreversible acknowledgement is needed. */
export async function confirmCleanup(scanId: string, selected: ScanItem[], options: { allowPermanent?: boolean } = {}): Promise<CleanupRun | undefined> {
  const allowPermanent = options.allowPermanent ?? true;
  let permanentChosen = false;
  let preview = await api.preview(scanId, selected.map((item) => item.id), permanentChosen);
  const dialog = el("dialog");
  dialog.className = "dialog";
  let ackInput!: HTMLInputElement;
  const render = () => {
    const parts = buildCleanupDialog(preview, selected, allowPermanent, permanentChosen);
    dialog.replaceChildren(...parts.nodes);
    ackInput = parts.ackInput;
    const permanent = parts.permanentInput;
    if (permanent) {
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
      const acknowledged = !preview.requiresIrreversibleAck || ackInput.checked;
      if (!acknowledged) throw new Error("Permanent cleanup requires acknowledgement");
      run = await api.run(preview.id, acknowledged);
    }
  } finally { dialog.remove(); }
  return run;
}
