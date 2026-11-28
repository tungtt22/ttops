// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

import type { Scan, ScanItem } from "../../api";
import { render } from "./view";

const item = (id: string, category: ScanItem["category"]): ScanItem => ({
  id, ruleId: "r", category, label: id, sizeBytes: 1,
  selectable: true, preselected: false, permanentOnly: false, details: [],
});

function mount(items: ScanItem[]) {
  const root = document.createElement("div");
  root.innerHTML = `<button id="clean" disabled></button><p id="status"></p><div id="results"></div>`;
  document.body.replaceChildren(root);
  const scan: Scan = { id: "s", status: "Complete", items, unscanned: [] };
  render(root, scan);
  const clean = root.querySelector<HTMLButtonElement>("#clean")!;
  const box = (id: string) => root.querySelector<HTMLInputElement>(`input[data-id="${id}"]`)!;
  const button = (text: string) => Array.from(root.querySelectorAll("button")).find((b) => b.textContent === text)!;
  return { clean, box, button };
}

const toggle = (box: HTMLInputElement) => { box.checked = !box.checked; box.dispatchEvent(new Event("change")); };

describe("storage selection enables Clean selected", () => {
  it("tracks individual B and C checkboxes when nothing is preselected", () => {
    const { clean, box } = mount([item("b1", "B"), item("c1", "C")]);
    expect(clean.disabled).toBe(true);
    toggle(box("b1"));
    expect(clean.disabled).toBe(false);
    toggle(box("b1"));
    expect(clean.disabled).toBe(true);
    toggle(box("c1"));
    expect(clean.disabled).toBe(false);
  });

  it("tracks B Select all / Select none", () => {
    const { clean, button } = mount([item("b1", "B"), item("b2", "B")]);
    button("Select all").click();
    expect(clean.disabled).toBe(false);
    button("Select none").click();
    expect(clean.disabled).toBe(true);
  });
});
