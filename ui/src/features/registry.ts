import { storageView } from "./storage/view";

export interface Feature { id: string; label: string; mount: (root: HTMLElement) => void | Promise<void>; }
const historyView = (root: HTMLElement) => { root.innerHTML = "<section><h1>History</h1><p>Cleanup history will appear here after the first completed run.</p></section>"; };
export const features: Feature[] = [
  { id: "storage", label: "Storage", mount: storageView },
  { id: "history", label: "History", mount: historyView },
];
