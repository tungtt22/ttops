// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

import type { Preview, ScanItem } from "../../api";
import { buildCleanupDialog } from "./clean";

const PAYLOAD = `<img src=x onerror="window.__pwned=1"><b>bold</b>`;

const scanItem = (id: string, label: string): ScanItem => ({ id, label } as ScanItem);

const preview = (overrides: Partial<Preview> = {}): Preview => ({
  id: "p", totalBytes: 10, requiresIrreversibleAck: false,
  items: [{ itemId: "a", label: PAYLOAD, sizeBytes: 10, action: "Trash", details: [PAYLOAD] }],
  refused: [{ itemId: "b", reason: PAYLOAD }],
  ...overrides,
});

function mount(parts: { nodes: Node[] }): HTMLDialogElement {
  const dialog = document.createElement("dialog");
  dialog.replaceChildren(...parts.nodes);
  document.body.append(dialog);
  return dialog;
}

describe("cleanup preview dialog", () => {
  it("renders markup-bearing labels, details and refusal reasons literally", () => {
    const dialog = mount(buildCleanupDialog(preview(), [scanItem("b", PAYLOAD)], true, false));
    expect(dialog.querySelector("img")).toBeNull();
    expect(dialog.querySelector("b")).toBeNull();
    expect(dialog.querySelector("[onerror]")).toBeNull();
    expect(dialog.textContent).toContain(PAYLOAD);
    expect(dialog.querySelector("code")!.textContent).toBe(PAYLOAD);
    expect((window as unknown as { __pwned?: number }).__pwned).toBeUndefined();
    dialog.remove();
  });

  it("keeps the confirmation controls", () => {
    const safe = preview({ items: [{ itemId: "a", label: "cache", sizeBytes: 1, action: "Permanent", details: [] }], refused: [], requiresIrreversibleAck: true });
    const parts = buildCleanupDialog(safe, [], true, true);
    const dialog = mount(parts);
    expect(parts.permanentInput!.checked).toBe(true);
    expect(dialog.querySelector<HTMLElement>("#ack")!.hidden).toBe(false);
    expect(Array.from(dialog.querySelectorAll("button")).map((b) => b.value)).toEqual(["cancel", "confirm"]);

    const none = buildCleanupDialog(preview({ items: [], refused: [] }), [], false, false);
    expect(none.permanentInput).toBeUndefined();
    const confirm = mount(none).querySelector<HTMLButtonElement>('button[value="confirm"]')!;
    expect(confirm.disabled).toBe(true);
  });
});
