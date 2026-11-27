import { api, type StartupItem, type StartupScope } from "../../api";

const SCOPES: Record<StartupScope, string> = { userAgent: "Your launch agents", systemAgent: "System launch agents (read-only)", systemDaemon: "System daemons (read-only)" };

/** Startup items: user LaunchAgents can be disabled or enabled after confirmation. */
export async function startupView(root: HTMLElement): Promise<void> {
  root.innerHTML = `<section><h1>Startup</h1><p>Background items that start when you log in. Disabling your own items is reversible and takes effect at next login. Login items added by apps are managed in System Settings → General → Login Items.</p><p id="status"></p><div id="results"></div></section>`;
  const status = root.querySelector<HTMLElement>("#status")!;
  const results = root.querySelector<HTMLElement>("#results")!;
  const draw = (items: StartupItem[]) => {
    results.replaceChildren(...(Object.keys(SCOPES) as StartupScope[]).map((scope) => {
      const card = document.createElement("section"); card.className = "card";
      const list = items.filter((item) => item.scope === scope);
      const heading = document.createElement("h2"); heading.textContent = `${SCOPES[scope]} · ${list.length}`; card.append(heading);
      for (const item of list) {
        const row = document.createElement("div"); row.className = "item";
        row.append(document.createTextNode(`${item.enabled ? "●" : "○"} ${item.label}${item.program ? ` — ${item.program}` : ""} `));
        if (item.editable) {
          const toggle = document.createElement("button"); toggle.type = "button";
          toggle.textContent = item.enabled ? "Disable" : "Enable";
          toggle.onclick = async () => {
            const verb = item.enabled ? "disable" : "enable";
            if (!window.confirm(`${verb[0].toUpperCase()}${verb.slice(1)} "${item.label}"? You can change this back at any time.`)) return;
            try { await api.setStartupEnabled(item.label, !item.enabled, true); status.textContent = `${item.label}: ${verb}d.`; await refresh(); }
            catch (error) { status.textContent = String(error); }
          };
          row.append(toggle);
        }
        card.append(row);
      }
      return card;
    }));
  };
  const refresh = async () => {
    try { draw(await api.startupItems()); } catch (error) { status.textContent = String(error); }
  };
  await refresh();
}
