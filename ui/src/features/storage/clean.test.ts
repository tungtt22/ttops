// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

import type { ScanItem } from "../../api";
import { buildCleanupDialog } from "./clean";

const item = (label: string, permanentOnly = false): ScanItem => ({
  id: label, ruleId: "rule", category: "A", label, sizeBytes: 10,
  selectable: true, preselected: true, permanentOnly,
});

describe("cleanup preview dialog", () => {
  it("renders markup-bearing filesystem labels literally", () => {
    const payload = `<img src=x onerror="window.__pwned=1"><b>bold</b>`;
    const { dialog } = buildCleanupDialog([item(payload)]);
    document.body.append(dialog);

    const entries = dialog.querySelectorAll("li");
    expect(entries).toHaveLength(1);
    expect(entries[0].textContent).toBe(payload);
    expect(entries[0].children).toHaveLength(0);
    expect(dialog.querySelector("img")).toBeNull();
    expect(dialog.querySelector("b")).toBeNull();
    expect(dialog.querySelector("[onerror]")).toBeNull();
    expect((window as unknown as { __pwned?: number }).__pwned).toBeUndefined();
    dialog.remove();
  });

  it("keeps the confirmation controls", () => {
    const normal = buildCleanupDialog([item("cache-a"), item("cache-b")]);
    expect(normal.dialog.querySelectorAll("li")).toHaveLength(2);
    expect(normal.permanentInput.checked).toBe(false);
    expect(normal.dialog.querySelector<HTMLElement>("#ack")!.hidden).toBe(true);
    normal.permanentInput.checked = true;
    normal.permanentInput.dispatchEvent(new Event("change"));
    expect(normal.dialog.querySelector<HTMLElement>("#ack")!.hidden).toBe(false);
    expect(Array.from(normal.dialog.querySelectorAll("button")).map((b) => b.value)).toEqual(["cancel", "confirm"]);

    const forced = buildCleanupDialog([item("docker", true)]);
    expect(forced.permanentInput.checked && forced.permanentInput.disabled).toBe(true);
    expect(forced.dialog.querySelector<HTMLElement>("#ack")!.hidden).toBe(false);
  });
});
